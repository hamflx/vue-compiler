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
  return native.parseSfc(String(source || ''), options || {});
}

function generateCodeFrame(source, start, end) {
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
