'use strict';

const native = require('@vuec-rs/native');

function compile(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  let result;
  if (source && typeof source === 'object' && source.type === 0 && Array.isArray(source.children)) {
    const payload = vue3AstCompilePayload(source, options);
    result = native.compileVue3Ssr(payload.source, payload.options);
  } else {
    const template = String(source || '');
    result = native.compileVue3Ssr(template, vue3SsrNativeOptions(options, template));
  }
  return hydrateCompileResult(result);
}

function hydrateCompileResult(result) {
  if (!result || typeof result !== 'object') return result;
  if (Array.isArray(result.ast_helpers)) {
    const helpers = new Set(result.ast_helpers.map(name => Symbol(name)));
    delete result.ast_helpers;
    result.ast = {
      ...(result.ast || {}),
      helpers,
    };
  }
  return result;
}

function vue3AstCompilePayload(ast, options) {
  const source = typeof ast.source === 'string' ? ast.source : '';
  const range = vue3AstChildrenRange(ast, source);
  const template = range ? source.slice(range.start, range.end) : source;
  return {
    source: template,
    options: Object.assign(vue3SsrNativeOptions(options, template), {
      __vuecTemplateBaseOffset: range ? range.start : 0,
      __vuecSourceMapSource: source,
      __vuecSourceMapBaseOffset: 0,
    }),
  };
}

function vue3SsrNativeOptions(options, source) {
  options = options || {};
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

module.exports = {
  compile,
};
