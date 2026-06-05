'use strict';

const cp = require('child_process');
const native = require('@vuec-rs/native');
let vue3CoreRuntime = null;

let packageVersion = '0.0.0-vuec-napi';
try {
  packageVersion = require('../package.json').version || packageVersion;
} catch (_) {}

let registeredTS = null;

class MagicString {
  constructor(original) {
    this.original = String(original || '');
    this.intro = '';
    this.outro = '';
    this.edits = [];
  }

  append(content) {
    this.outro += String(content);
    return this;
  }

  prepend(content) {
    this.intro = String(content) + this.intro;
    return this;
  }

  overwrite(start, end, content) {
    this.edits.push({ start: Number(start) || 0, end: Number(end) || 0, content: String(content) });
    return this;
  }

  remove(start, end) {
    return this.overwrite(start, end, '');
  }

  slice(start, end) {
    return this.original.slice(start, end);
  }

  toString() {
    const sorted = this.edits.slice().sort((a, b) => a.start - b.start || a.end - b.end);
    let cursor = 0;
    let output = this.intro;
    for (const edit of sorted) {
      const start = Math.max(0, Math.min(this.original.length, edit.start));
      const end = Math.max(start, Math.min(this.original.length, edit.end));
      if (start < cursor) {
        continue;
      }
      output += this.original.slice(cursor, start);
      output += edit.content;
      cursor = end;
    }
    output += this.original.slice(cursor);
    output += this.outro;
    return output;
  }

  generateMap() {
    return { version: 3, sources: [], names: [], mappings: '', sourcesContent: [] };
  }
}

MagicString.Bundle = undefined;
MagicString.SourceMap = undefined;
MagicString.default = MagicString;

function parse(source, options) {
  const parser = loadBabelParser();
  if (parser && typeof parser.parse === 'function') {
    return parser.parse(source, options);
  }
  return minimalBabelParse(source, options);
}

function parse$1(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  const payload = {
    source: String(source || ''),
    filename: options && options.filename,
    options: options || {},
  };
  const bridgePayload = vue3SfcParseBridgePayload(payload);
  const parsed = hasBridge()
    ? callBridge('sfc.parse', bridgePayloadForCall(bridgePayload))
    : native.parseSfcResult(payload.source, bridgePayload.bridgeOptions || {});
  return hydrateVue3SfcParseResult(
    applyVue3SfcCustomCompilerParse(parsed, payload.source, payload.options, payload.filename)
  );
}

function compileTemplate(options) {
  const opts = options || {};
  const payload = {
    source: String(opts.source || ''),
    filename: opts.filename || 'template.vue.html',
    options: opts,
  };
  const customResult = vue3SfcCustomCompileTemplateResult(payload);
  if (customResult !== undefined) {
    return customResult;
  }
  const bridgePayload = vue3SfcCompileTemplateBridgePayload(payload);
  if (hasBridge()) {
    return hydrateVue3SfcCompileTemplateResult(
      callBridge('sfc.compileTemplate', bridgePayloadForCall(bridgePayload))
    );
  }
  return hydrateVue3SfcCompileTemplateResult(
    native.compileTemplate(vue3SfcCompileTemplateNativeOptions(bridgePayload))
  );
}

function compileScript(descriptor, options) {
  const payload = {
    source: descriptor && typeof descriptor.source === 'string' ? descriptor.source : '',
    filename: descriptor && descriptor.filename,
    options: options || {},
  };
  const bridgePayload = vue3CompileScriptBridgePayload(payload);
  if (hasBridge()) {
    return hydrateVue3CompileScriptResult(
      callBridge('sfc.compileScript', bridgePayloadForCall(bridgePayload))
    );
  }
  return hydrateVue3CompileScriptResult(
    native.compileScript(descriptor || {}, bridgePayload.options || {})
  );
}

function compileStyle(options) {
  const opts = options || {};
  return normalizeVue3StyleResult(
    emitVue3StyleWarnings(
      native.compileStyle(vue3StyleNativeOptions(resolveStylePreprocessOptions(String(opts.source || ''), opts)))
    )
  );
}

function compileStyleAsync(options) {
  return Promise.resolve(compileStyle(options || {}));
}

function emitVue3StyleWarnings(result) {
  if (!result || !Array.isArray(result.diagnostics) || !result.diagnostics.length) {
    return result;
  }
  const diagnostics = [];
  for (const diagnostic of result.diagnostics) {
    const severity = diagnostic && diagnostic.severity;
    const code = diagnostic && diagnostic.code;
    const message = typeof diagnostic === 'string' ? diagnostic : diagnostic && diagnostic.message;
    if (
      severity === 'warning' &&
      code === 'VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR' &&
      message
    ) {
      console.warn(`[@vue/compiler-sfc] ${message}`);
    } else {
      diagnostics.push(diagnostic);
    }
  }
  if (diagnostics.length === result.diagnostics.length) {
    return result;
  }
  const out = { ...result };
  if (diagnostics.length) {
    out.diagnostics = diagnostics;
  } else {
    delete out.diagnostics;
  }
  return out;
}

function vue3StyleNativeOptions(options) {
  if (!options || typeof options !== 'object') return options;
  const out = { ...options };
  delete out.sourceMap;
  delete out.source_map;
  return out;
}

function normalizeVue3StyleResult(result) {
  if (!result || typeof result !== 'object' || result.map !== null) return result;
  const out = { ...result };
  out.map = undefined;
  return out;
}

function vue3CompileScriptBridgePayload(payload) {
  const out = { ...(payload || {}) };
  const options = { ...(out.options || {}) };
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

function hasBridge() {
  return !!process.env.VUEC_NODE_BRIDGE;
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

function vue3SfcParseBridgePayload(payload) {
  const out = { ...(payload || {}) };
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
      source
    );
  }
  return normalized;
}

function applyVue3SfcCustomCompilerParse(result, source, options, filename) {
  if (!result || typeof result !== 'object' || !result.descriptor) return result;
  const compiler = options && options.compiler;
  if (!compiler || typeof compiler.parse !== 'function') return result;
  const customErrors = [];
  const ast = compiler.parse(String(source || ''), {
    ...(options.templateParseOptions || {}),
    parseMode: 'sfc',
    prefixIdentifiers: true,
    onError: error => customErrors.push(error),
  });
  const out = { ...result };
  out.errors = customErrors.concat(Array.isArray(result.errors) ? result.errors : []);
  if (ast && Array.isArray(ast.children) && ast.children.length === 0) {
    out.errors.push(new SyntaxError(
      `At least one <template> or <script> is required in a single file component. ${filename || 'anonymous.vue'}`
    ));
  }
  return out;
}

function vue3SfcCompileTemplateBridgePayload(payload) {
  const out = { ...(payload || {}) };
  const options = { ...(out.options || {}) };
  const source = String(out.source || '');
  const bridgeOptions = vue3SfcCompileTemplateOptionsForBridge(options, source);
  if (options.ast) {
    out.ast = dehydrateForBridge(options.ast);
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
  const bridgeOptions = { ...normalizeVue3OptionsForBridge(options, source) };
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
  if (options && options.ssrCssVars !== undefined) bridgeOptions.ssrCssVars = options.ssrCssVars;
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

function vue3SfcCompileTemplateNativeOptions(payload) {
  const bridgeOptions = payload && payload.bridgeOptions && typeof payload.bridgeOptions === 'object'
    ? payload.bridgeOptions
    : {};
  const options = payload && payload.options && typeof payload.options === 'object'
    ? payload.options
    : {};
  return {
    ...options,
    ...bridgeOptions,
    source: String(payload && payload.source || ''),
    filename: payload && payload.filename,
  };
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

function getVue3CoreRuntime() {
  if (vue3CoreRuntime) return vue3CoreRuntime;
  try {
    const core = require('@vue/compiler-core');
    vue3CoreRuntime = core.__vuecRuntime || core;
  } catch (_) {
    vue3CoreRuntime = {};
  }
  return vue3CoreRuntime;
}

function hydrateVue3Ast(ast, options) {
  const runtime = getVue3CoreRuntime();
  return typeof runtime.hydrateVue3Ast === 'function'
    ? runtime.hydrateVue3Ast(ast, options || {})
    : ast;
}

function dehydrateForBridge(value) {
  const runtime = getVue3CoreRuntime();
  if (typeof runtime.dehydrateForBridge === 'function') {
    return runtime.dehydrateForBridge(value);
  }
  return value;
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

function hydrateVue3SfcCompileTemplateResult(result) {
  if (!result || typeof result !== 'object') return result;
  const out = { ...result };
  delete out.ast_summary;
  delete out.astSummary;
  delete out.bindings;
  if (typeof out.ast === 'string') {
    try {
      out.ast = JSON.parse(out.ast);
    } catch (_) {}
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
  if (node.type === 1) {
    let tag = String(node.tag || '');
    if (tag.includes('.')) tag = tag.split('.')[0].trim();
    if (tag && !vue3SfcIsNativeTag(tag) && !vue3SfcIsDomBuiltInComponent(tag)) {
      ids.add(camelize(tag));
      ids.add(capitalize(camelize(tag)));
    }
    for (const prop of node.props || []) {
      if (prop && prop.type === 7) {
        if (!vue3SfcIsBuiltInDirective(prop.name)) {
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
      } else if (prop && prop.type === 6 && prop.name === 'ref' && prop.value && prop.value.content) {
        ids.add(prop.value.content);
      }
    }
    for (const child of node.children || []) {
      collectVue3SfcTemplateIds(child, ids);
    }
  } else if (node.type === 5) {
    collectVue3SfcExpressionIds(node.content, ids);
  }
}

function collectVue3SfcExpressionIds(exp, ids) {
  if (!exp) return;
  if (exp.ast) {
    collectVue3SfcAstIds(exp.ast, ids);
  } else if (exp.ast === null) {
    collectVue3SfcStringExpressionIds(exp.content, ids);
  } else if (exp.content) {
    collectVue3SfcStringExpressionIds(exp.content, ids);
  }
}

function collectVue3SfcAstIds(root, ids) {
  walkAst(root, (node, parent) => {
    if (!node || node.type !== 'Identifier') return;
    if (parent && parent.type === 'MemberExpression' && parent.property === node && !parent.computed) return;
    if (parent && (parent.type === 'ObjectProperty' || parent.type === 'Property') && parent.key === node && !parent.computed) return;
    ids.add(node.name);
  });
}

function collectVue3SfcStringExpressionIds(source, ids) {
  const text = String(source || '');
  const pattern = /[A-Za-z_$][\w$]*/g;
  let match;
  while ((match = pattern.exec(text))) {
    const name = match[0];
    const before = text.slice(0, match.index).trimEnd();
    if (before.endsWith('.')) continue;
    ids.add(name);
  }
}

function vue3SfcIsBuiltInDirective(name) {
  return new Set(['bind', 'cloak', 'else-if', 'else', 'for', 'html', 'if', 'model', 'on', 'once', 'pre', 'show', 'slot', 'text', 'memo']).has(String(name || ''));
}

function vue3SfcIsNativeTag(tag) {
  return /^(?:html|body|base|head|link|meta|style|title|address|article|aside|footer|header|hgroup|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot|svg|math)$/i.test(String(tag || ''));
}

function vue3SfcIsDomBuiltInComponent(tag) {
  return tag === 'Transition' || tag === 'transition' || tag === 'TransitionGroup' || tag === 'transition-group';
}

function capitalize(value) {
  value = String(value || '');
  return value ? value.charAt(0).toUpperCase() + value.slice(1) : value;
}

function camelize(value) {
  return String(value || '').replace(/-(\w)/g, (_, c) => c ? c.toUpperCase() : '');
}

function resolveStylePreprocessOptions(source, options) {
  if (!options || !options.preprocessOptions || typeof options.preprocessOptions !== 'object') {
    return options;
  }
  const preprocessOptions = options.preprocessOptions;
  if (typeof preprocessOptions.additionalData !== 'function') {
    return options;
  }
  return {
    ...options,
    preprocessOptions: {
      ...preprocessOptions,
      additionalData: preprocessOptions.additionalData(source, options.filename),
    },
  };
}

function generateCodeFrame(source) {
  const start = arguments.length > 1 ? arguments[1] : undefined;
  const end = arguments.length > 2 ? arguments[2] : undefined;
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

function rewriteDefault(source, as, parserPlugins) {
  return native.rewriteDefaultVue3(String(source || ''), String(as || ''), parserPlugins || []);
}

function rewriteDefaultAST(ast, s, as) {
  const body = Array.isArray(ast) ? ast : ast && ast.program && Array.isArray(ast.program.body) ? ast.program.body : [];
  let found = false;
  for (const node of body) {
    if (node && node.type === 'ExportDefaultDeclaration') {
      found = true;
      const declaration = node.declaration || {};
      if (declaration.type === 'ClassDeclaration' && declaration.id && declaration.id.name) {
        const start = Array.isArray(declaration.decorators) && declaration.decorators.length
          ? declaration.decorators[declaration.decorators.length - 1].end
          : node.start;
        s.overwrite(start || node.start || 0, declaration.id.start || node.start || 0, ' class ');
        s.append(`\nconst ${as} = ${declaration.id.name}`);
      } else {
        s.overwrite(node.start || 0, declaration.start || node.start || 0, `const ${as} = `);
      }
    } else if (node && node.type === 'ExportNamedDeclaration' && Array.isArray(node.specifiers)) {
      for (const specifier of node.specifiers) {
        const exported = specifier && specifier.exported;
        if (exported && exported.type === 'Identifier' && exported.name === 'default') {
          found = true;
          const local = specifier.local && specifier.local.name ? specifier.local.name : 'default';
          if (node.source && node.source.value) {
            s.prepend(`import { ${local} as __VUE_DEFAULT__ } from '${node.source.value}'\n`);
            s.append(`\nconst ${as} = __VUE_DEFAULT__`);
          } else {
            s.append(`\nconst ${as} = ${local}`);
          }
          s.remove(specifier.start || node.start || 0, specifier.end || node.end || 0);
        }
      }
    }
  }
  if (!found) {
    s.append(`\nconst ${as} = {}`);
  }
}

function extractIdentifiers(param) {
  const out = [];
  collectIdentifiers(param, out, { includeMembers: false });
  return out;
}

function walkIdentifiers(root, onIdentifier) {
  const includeAll = arguments.length > 2 ? arguments[2] : false;
  const projection = native.callVue3CoreProjection('vue3.core.walkIdentifiers', {
    root,
    includeAll,
  });
  for (const event of projection.identifiers || []) {
    const node = nodeAtPath(root, event.path);
    if (!node) continue;
    const parent = event.parentPath ? nodeAtPath(root, event.parentPath) : null;
    const stack = (event.parentStackPaths || [])
      .map(path => nodeAtPath(root, path))
      .filter(Boolean);
    onIdentifier(node, parent, stack, !!event.isReferenced, !!event.isLocal);
  }
}

function walk(root, enter) {
  const leave = arguments.length > 2 ? arguments[2] : undefined;
  return walkAst(root, (node, parent) => {
    if (typeof enter === 'function') enter(node, parent);
  }, (node, parent) => {
    if (typeof leave === 'function') leave(node, parent);
  });
}

function extractRuntimeProps(ctx) {
  if (!ctx || !ctx.propsTypeDecl) {
    return ctx && ctx.propsRuntimeDecl ? stringifyNode(ctx.propsRuntimeDecl, ctx) : undefined;
  }
  const elements = resolveTypeElements(ctx, ctx.propsTypeDecl);
  const props = Object.keys(elements.props || {});
  if (!props.length) return undefined;
  return `{ ${props.map(name => `${JSON.stringify(name)}: { type: null }`).join(', ')} }`;
}

function extractRuntimeEmits(ctx) {
  if (!ctx || !ctx.emitsTypeDecl) {
    return ctx && ctx.emitsRuntimeDecl ? stringifyNode(ctx.emitsRuntimeDecl, ctx) : undefined;
  }
  const elements = resolveTypeElements(ctx, ctx.emitsTypeDecl);
  const names = Object.keys(elements.props || {});
  return names.length ? `[${names.map(JSON.stringify).join(', ')}]` : undefined;
}

function inferRuntimeType(ctx, node) {
  switch (node && node.type) {
    case 'TSStringKeyword':
    case 'StringLiteral':
      return ['String'];
    case 'TSNumberKeyword':
    case 'NumericLiteral':
    case 'BigIntLiteral':
      return ['Number'];
    case 'TSBooleanKeyword':
    case 'BooleanLiteral':
      return ['Boolean'];
    case 'TSArrayType':
    case 'TSTupleType':
      return ['Array'];
    case 'TSFunctionType':
    case 'TSCallSignatureDeclaration':
    case 'TSMethodSignature':
      return ['Function'];
    case 'TSTypeLiteral':
    case 'TSInterfaceDeclaration':
    case 'TSObjectKeyword':
    case 'ClassDeclaration':
      return ['Object'];
    case 'TSNullKeyword':
      return ['null'];
    case 'TSLiteralType':
      return inferRuntimeType(ctx, node.literal);
    case 'TSUnionType': {
      const set = new Set();
      for (const ty of node.types || []) {
        for (const item of inferRuntimeType(ctx, ty)) set.add(item);
      }
      return Array.from(set);
    }
    case 'TSIntersectionType':
      return Array.from(new Set([].concat(...(node.types || []).map(ty => inferRuntimeType(ctx, ty))))).filter(type => type !== 'Unknown');
    case 'TSTypeReference': {
      const name = getReferenceName(node);
      if (['Array', 'Function', 'Object', 'Set', 'Map', 'WeakSet', 'WeakMap', 'Date', 'Promise', 'Error'].includes(name)) {
        return [name];
      }
      if (['Partial', 'Required', 'Readonly', 'Record', 'Pick', 'Omit', 'InstanceType'].includes(name)) {
        return ['Object'];
      }
      if (['Parameters', 'ConstructorParameters', 'ReadonlyArray'].includes(name)) {
        return ['Array'];
      }
      if (['Uppercase', 'Lowercase', 'Capitalize', 'Uncapitalize'].includes(name)) {
        return ['String'];
      }
      return ['Unknown'];
    }
    default:
      return ['Unknown'];
  }
}

function resolveTypeElements(ctx, node, scope, typeParameters) {
  const props = Object.create(null);
  const calls = [];
  const members = node && node.type === 'TSTypeLiteral'
    ? node.members || []
    : node && node.type === 'TSInterfaceDeclaration' && node.body
      ? node.body.body || []
      : [];
  for (const member of members) {
    if (!member) continue;
    if (member.type === 'TSCallSignatureDeclaration' || member.type === 'TSFunctionType') {
      calls.push(member);
      continue;
    }
    if (member.type === 'TSPropertySignature' || member.type === 'TSMethodSignature') {
      const name = staticKeyName(member.key);
      if (name != null) props[name] = member;
    }
  }
  const result = { props };
  if (calls.length) result.calls = calls;
  return result;
}

function invalidateTypeCache(ctx) {
  for (const key of Object.keys(parseCache)) {
    parseCache[key] = undefined;
  }
}

function registerTS(ts) {
  registeredTS = ts || null;
}

function namedNoPrototype(name, arity, fn) {
  const bound = fn.bind(null);
  Object.defineProperty(bound, 'name', { value: name, configurable: true });
  Object.defineProperty(bound, 'length', { value: arity, configurable: true });
  return bound;
}

const isStaticProperty = namedNoPrototype('isStaticProperty', 1, function (node) {
  return !!(node && (node.type === 'ObjectProperty' || node.type === 'Property' || node.type === 'ObjectMethod' || node.type === 'ClassProperty' || node.type === 'PropertyDefinition') && !node.computed);
});

function isInDestructureAssignment(parent, parentStack) {
  if (!parent) return false;
  if (parent.type === 'ObjectPattern' || parent.type === 'ArrayPattern') return true;
  return Array.isArray(parentStack) && parentStack.some(node => node && (node.type === 'ObjectPattern' || node.type === 'ArrayPattern'));
}

const shouldTransformRef = namedNoPrototype('shouldTransformRef', 0, function () {
  return false;
});

const parseCache = {};
[
  'allowStale',
  'allowStaleOnFetchAbort',
  'allowStaleOnFetchRejection',
  'ignoreFetchAbort',
  'maxEntrySize',
  'noDeleteOnFetchRejection',
  'noDeleteOnStaleGet',
  'noDisposeOnSet',
  'noUpdateTTL',
  'sizeCalculation',
  'ttl',
  'ttlAutopurge',
  'ttlResolution',
  'updateAgeOnGet',
  'updateAgeOnHas',
].forEach(key => {
  parseCache[key] = undefined;
});

const errorMessages = {};
for (let i = 0; i <= 64; i++) {
  errorMessages[i] = `compiler error ${i}`;
}

function loadBabelParser() {
  try {
    return require('@babel/parser');
  } catch (_) {
    return null;
  }
}

function minimalBabelParse(source, options) {
  const text = String(source || '');
  return {
    type: 'File',
    program: {
      type: 'Program',
      sourceType: options && options.sourceType ? options.sourceType : 'module',
      body: parseTopLevelStatements(text),
    },
  };
}

function parseTopLevelStatements(source) {
  const statements = [];
  const exportDefault = /\bexport\s+default\s+/g;
  let match;
  while ((match = exportDefault.exec(source))) {
    const declarationStart = match.index + match[0].length;
    const declaration = parseDefaultDeclaration(source, declarationStart);
    statements.push({
      type: 'ExportDefaultDeclaration',
      start: match.index,
      end: declaration.end,
      declaration,
    });
  }

  const exportNamed = /\bexport\s*\{([\s\S]*?)\}(?:\s*from\s*(['"])(.*?)\2)?/g;
  while ((match = exportNamed.exec(source))) {
    const specifiers = [];
    let cursor = match.index + match[0].indexOf('{') + 1;
    for (const rawPart of match[1].split(',')) {
      const part = rawPart.trim();
      if (!part) {
        cursor += rawPart.length + 1;
        continue;
      }
      const asMatch = part.match(/^(.+?)\s+as\s+(.+)$/);
      const local = (asMatch ? asMatch[1] : part).trim();
      const exported = (asMatch ? asMatch[2] : part).trim();
      const partStart = source.indexOf(part, cursor);
      const localStart = partStart + part.indexOf(local);
      const exportedStart = partStart + part.lastIndexOf(exported);
      specifiers.push({
        type: 'ExportSpecifier',
        start: partStart,
        end: partStart + part.length,
        local: identifierNode(local, localStart),
        exported: identifierNode(exported, exportedStart),
      });
      cursor = partStart + part.length + 1;
    }
    statements.push({
      type: 'ExportNamedDeclaration',
      start: match.index,
      end: match.index + match[0].length,
      specifiers,
      source: match[3] ? { type: 'StringLiteral', value: match[3] } : null,
    });
  }
  statements.sort((a, b) => a.start - b.start);
  return statements;
}

function parseDefaultDeclaration(source, start) {
  const tail = source.slice(start);
  const classMatch = tail.match(/^\s*class\s+([A-Za-z_$][\w$]*)/);
  if (classMatch) {
    const classStart = start + classMatch[0].indexOf('class');
    const nameStart = start + classMatch[0].lastIndexOf(classMatch[1]);
    return {
      type: 'ClassDeclaration',
      start: classStart,
      end: findStatementEnd(source, start),
      id: identifierNode(classMatch[1], nameStart),
      decorators: [],
    };
  }
  return {
    type: 'Expression',
    start,
    end: findStatementEnd(source, start),
  };
}

function findStatementEnd(source, start) {
  const semicolon = source.indexOf(';', start);
  const newline = source.indexOf('\n', start);
  const end = [semicolon, newline].filter(index => index >= 0).sort((a, b) => a - b)[0];
  return end == null ? source.length : end + (end === semicolon ? 1 : 0);
}

function identifierNode(name, start) {
  return {
    type: 'Identifier',
    name,
    start,
    end: start + String(name).length,
  };
}

function collectIdentifiers(node, out, options) {
  walkAst(node, current => {
    if (current && current.type === 'Identifier') {
      out.push(current);
    } else if (options && options.includeMembers && current && current.type === 'MemberExpression') {
      collectIdentifiers(current.object, out, options);
    }
  });
}

function walkAst(root, enter, leave, parent, stack) {
  if (!root || typeof root !== 'object') return root;
  const parents = stack || [];
  if (Array.isArray(root)) {
    for (const item of root) walkAst(item, enter, leave, parent, parents);
    return root;
  }
  if (enter) enter(root, parent || null, parents);
  const nextStack = parent ? parents.concat(parent) : parents;
  for (const key of Object.keys(root)) {
    if (key === 'parent' || key === 'loc') continue;
    const value = root[key];
    if (value && typeof value === 'object') {
      walkAst(value, enter, leave, root, nextStack);
    }
  }
  if (leave) leave(root, parent || null, parents);
  return root;
}

function nodeAtPath(root, path) {
  let node = root;
  for (const segment of path || []) {
    if (node == null) return null;
    node = node[segment];
  }
  return node || null;
}

function isBindingIdentifier(node, parent) {
  if (!node || !parent) return false;
  if (parent.id === node && /Declaration$/.test(parent.type || '')) return true;
  if ((parent.type === 'ObjectProperty' || parent.type === 'Property') && parent.key === node && !parent.computed) return false;
  return parent.type === 'ObjectPattern' || parent.type === 'ArrayPattern' || parent.type === 'RestElement' || parent.type === 'AssignmentPattern';
}

function getReferenceName(node) {
  const typeName = node && node.typeName;
  if (!typeName) return '';
  if (typeName.type === 'Identifier') return typeName.name;
  if (typeName.type === 'TSQualifiedName') return `${getReferenceName({ typeName: typeName.left })}.${typeName.right && typeName.right.name}`;
  return '';
}

function staticKeyName(node) {
  if (!node) return null;
  if (node.type === 'Identifier') return node.name;
  if (node.type === 'StringLiteral' || node.type === 'NumericLiteral') return String(node.value);
  return null;
}

function stringifyNode(node, ctx) {
  if (!node) return undefined;
  if (typeof node === 'string') return node;
  if (typeof ctx.getString === 'function' && node.start != null && node.end != null) {
    return ctx.getString(node);
  }
  if (node.content) return String(node.content);
  return undefined;
}

function callBridge(command, payload) {
  const bridgeBin = process.env.VUEC_NODE_BRIDGE;
  if (!bridgeBin) {
    throw new Error('VUEC_NODE_BRIDGE is required for Vue compiler-sfc conformance bridge calls');
  }
  const result = cp.spawnSync(bridgeBin, [String(command || '')], {
    input: JSON.stringify(payload || {}),
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const error = new Error(result.stderr || result.stdout || `vuec bridge command failed: ${command}`);
    error.code = 'VUEC_BRIDGE_FAILED';
    throw error;
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : undefined;
}

module.exports = {
  MagicString,
  babelParse: parse,
  compileScript,
  compileStyle,
  compileStyleAsync,
  compileTemplate,
  errorMessages,
  extractIdentifiers,
  extractRuntimeEmits,
  extractRuntimeProps,
  generateCodeFrame,
  inferRuntimeType,
  invalidateTypeCache,
  isInDestructureAssignment,
  isStaticProperty,
  parse: parse$1,
  parseCache,
  registerTS,
  resolveTypeElements,
  rewriteDefault,
  rewriteDefaultAST,
  shouldTransformRef,
  version: packageVersion,
  walk,
  walkIdentifiers,
};

Object.defineProperty(module.exports, '__vuecRuntime', {
  value: { callBridge },
  enumerable: false,
});
