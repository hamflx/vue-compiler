'use strict';

const native = require('@vuec-rs/native');

function compile(template, options) {
  const result = normalizeVue2PublicCompileResult(native.compileVue2(String(template || ''), options || {}), options || {});
  emitVue2CompileWarnings(result, options || {});
  return result;
}

function compileToFunctions(template, options, vm) {
  void vm;
  const result = normalizeVue2PublicCompileResult(native.compileToFunctionsVue2(String(template || ''), options || {}), options || {});
  emitVue2CompileWarnings(result, options || {});
  return result;
}

const ssrCompile = function compile(template, options) {
  const result = normalizeVue2PublicCompileResult(native.compileSsrVue2(String(template || ''), options || {}), options || {});
  emitVue2CompileWarnings(result, options || {});
  return result;
};

const ssrCompileToFunctions = function compileToFunctions(template, options, vm) {
  void vm;
  const result = normalizeVue2PublicCompileResult(native.compileSsrVue2(String(template || ''), options || {}), options || {});
  emitVue2CompileWarnings(result, options || {});
  return result;
};

function parseComponent(source, options) {
  const opts = options || {};
  return normalizeVue27ParseComponentResult(
    native.parseVue27SfcComponent(String(source || ''), opts),
    opts,
  );
}

function generateCodeFrame(source) {
  const start = arguments.length > 1 ? arguments[1] : 0;
  const end = arguments.length > 2 ? arguments[2] : start;
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

function callBridge(command, payload) {
  return native.callVue2Bridge(command, payload || {});
}

function normalizeVue2PublicCompileResult(result, options) {
  if (!result || typeof result !== 'object') return result;
  const out = { ...result };
  out.staticRenderFns = Array.isArray(result.staticRenderFns)
    ? result.staticRenderFns
    : Array.isArray(result.static_render_fns)
      ? result.static_render_fns
      : [];
  const ranged = Boolean(options && options.outputSourceRange);
  out.errors = normalizeIssues(result.errors, ranged);
  out.tips = normalizeIssues(result.tips, ranged);
  delete out.diagnostics;
  delete out.static_render_fns;
  return out;
}

function normalizeIssues(issues, ranged) {
  if (!Array.isArray(issues)) return [];
  return issues.map(issue => {
    if (typeof issue === 'string') return ranged ? { msg: issue } : issue;
    if (!issue || typeof issue !== 'object') return ranged ? { msg: String(issue) } : String(issue);
    if (!ranged) return String(issue.msg || issue.message || issue);
    const out = { msg: String(issue.msg || issue.message || issue) };
    if (issue.start != null) out.start = issue.start;
    if (issue.end != null) out.end = issue.end;
    return out;
  });
}

function normalizeVue27ParseComponentResult(result, options) {
  if (!result || typeof result !== 'object') return result;
  const descriptor = normalizeVue27Descriptor(result.descriptor || result);
  descriptor.errors = normalizeIssues(result.errors, !!(options && options.outputSourceRange));
  return descriptor;
}

function normalizeVue27Descriptor(descriptor) {
  if (!descriptor || typeof descriptor !== 'object') return descriptor;
  return {
    source: descriptor.source || '',
    filename: descriptor.filename || 'anonymous.vue',
    template: descriptor.template ? normalizeVue27Block(descriptor, descriptor.template, false) : null,
    script: descriptor.script ? normalizeVue27Block(descriptor, descriptor.script, false) : null,
    scriptSetup: descriptor.script_setup ? normalizeVue27Block(descriptor, descriptor.script_setup, false) : null,
    styles: Array.isArray(descriptor.styles)
      ? descriptor.styles.map(block => normalizeVue27Block(descriptor, block, true))
      : [],
    customBlocks: Array.isArray(descriptor.custom_blocks)
      ? descriptor.custom_blocks.map(block => normalizeVue27Block(descriptor, block, false))
      : [],
    errors: [],
  };
}

function normalizeVue27Block(descriptor, block, style) {
  const out = {
    type: block.type_name || block.type || '',
    content: block.content || '',
    start: blockContentStart(block, descriptor),
    end: blockContentEnd(block, descriptor),
    attrs: vue27Attrs(block.attrs),
  };
  if (out.type === 'script' || out.type === 'style') {
    out.map = vue27BlockMap(descriptor);
  }
  if (block.attrs && block.attrs.setup) out.setup = true;
  if (block.attrs && block.attrs.lang) out.lang = block.attrs.lang;
  if (block.attrs && block.attrs.src) out.src = block.attrs.src;
  if (block.attrs && block.attrs.module != null) out.module = block.attrs.module === '' ? true : block.attrs.module;
  if (style && block.attrs && block.attrs.scoped) out.scoped = true;
  return out;
}

function vue27Attrs(attrs) {
  const raw = attrs && attrs.raw && typeof attrs.raw === 'object' ? attrs.raw : {};
  const out = {};
  for (const key of Object.keys(raw)) {
    out[key] = raw[key];
  }
  if (attrs && attrs.scoped) out.scoped = true;
  if (attrs && attrs.setup) out.setup = true;
  if (attrs && attrs.lang) out.lang = attrs.lang;
  if (attrs && attrs.src) out.src = attrs.src;
  if (attrs && attrs.module != null) out.module = attrs.module === '' ? true : attrs.module;
  return out;
}

function blockContentStart(block, descriptor) {
  if (typeof block.content_start === 'number') return block.content_start;
  const source = descriptor && typeof descriptor.source === 'string' ? descriptor.source : '';
  if (block && block.loc && typeof block.loc.start === 'number') {
    const openEnd = source.indexOf('>', block.loc.start);
    if (openEnd >= 0 && openEnd < block.loc.end) return openEnd + 1;
    return block.loc.start;
  }
  return 0;
}

function blockContentEnd(block, descriptor) {
  if (typeof block.content_end === 'number') return block.content_end;
  const start = blockContentStart(block, descriptor);
  if (block && typeof block.content === 'string') return start + block.content.length;
  return block && block.loc && typeof block.loc.end === 'number' ? block.loc.end : 0;
}

function vue27BlockMap(descriptor) {
  const filename = descriptor && descriptor.filename ? descriptor.filename : 'anonymous.vue';
  const source = descriptor && descriptor.source ? descriptor.source : '';
  return {
    version: 3,
    sources: [filename],
    names: [],
    mappings: 'AAAA',
    file: filename,
    sourceRoot: '',
    sourcesContent: [source],
  };
}

function emitVue2CompileWarnings(result, options) {
  const suppressed = options && options.__vuecSuppressWarnings;
  if (suppressed === true || !result || typeof result !== 'object') return;
  const suppressedMessages = Array.isArray(suppressed) ? suppressed.map(String) : [];
  for (const warning of [...(result.errors || []), ...(result.tips || [])]) {
    const message = typeof warning === 'string'
      ? warning
      : warning && typeof warning.msg === 'string'
        ? warning.msg
        : null;
    if (message == null) continue;
    if (suppressedMessages.some(suppressedMessage => message.includes(suppressedMessage))) continue;
    console.error(message);
  }
}

const api = {
  compile,
  compileToFunctions,
  ssrCompile,
  ssrCompileToFunctions,
  parseComponent,
  generateCodeFrame,
};

Object.defineProperty(api, '__vuecRuntime', {
  value: { callBridge },
  enumerable: false,
  configurable: true,
});

module.exports = api;
