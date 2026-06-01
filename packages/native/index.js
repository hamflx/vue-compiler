'use strict';

const fs = require('fs');
const path = require('path');

const loadedBinding = loadBinding();
const binding = loadedBinding.binding;

function fromJson(json) {
  return JSON.parse(json);
}

function loadBinding() {
  const attempts = [];
  const explicit = process.env.VUEC_NATIVE_BINDING_PATH;
  if (explicit) {
    return requireBindingPath(path.resolve(explicit), 'env', attempts);
  }

  const localPath = path.join(__dirname, 'vuec_napi.node');
  if (fs.existsSync(localPath)) {
    try {
      return requireBindingPath(localPath, 'local', attempts);
    } catch (_) {
      // Keep trying the optional platform package below.
    }
  }

  const packageName = platformPackageName();
  if (packageName) {
    try {
      return {
        binding: require(packageName),
        source: 'platform',
        path: require.resolve(packageName),
        package: packageName,
      };
    } catch (error) {
      attempts.push(`${packageName}: ${error.message}`);
    }
  }

  const message = [
    'Failed to load vuec_napi native binding.',
    `platform=${process.platform}`,
    `arch=${process.arch}`,
    attempts.length ? `attempts=${attempts.join(' | ')}` : 'attempts=none',
  ].join(' ');
  throw new Error(message);
}

function requireBindingPath(bindingPath, source, attempts) {
  try {
    return {
      binding: require(bindingPath),
      source,
      path: bindingPath,
      package: null,
    };
  } catch (error) {
    attempts.push(`${bindingPath}: ${error.message}`);
    throw error;
  }
}

function platformPackageName() {
  const platform = process.platform;
  const arch = process.arch;
  if (platform === 'win32' && (arch === 'x64' || arch === 'arm64')) {
    return `@vuec-rs/native-win32-${arch}`;
  }
  if (platform === 'darwin' && (arch === 'x64' || arch === 'arm64')) {
    return `@vuec-rs/native-darwin-${arch}`;
  }
  if (platform === 'linux' && (arch === 'x64' || arch === 'arm64')) {
    return `@vuec-rs/native-linux-${arch}-${isMusl() ? 'musl' : 'gnu'}`;
  }
  return null;
}

function isMusl() {
  if (process.platform !== 'linux') {
    return false;
  }
  const report = typeof process.report === 'object' && process.report
    && typeof process.report.getReport === 'function'
    ? process.report.getReport()
    : null;
  return !report || !report.header || !report.header.glibcVersionRuntime;
}

function compileVue2(template, options = {}) {
  return normalizeVue2CompileResult(fromJson(binding.compileVue2(template, dehydrateForNative(options || {}))));
}

function compileToFunctionsVue2(template, options = {}) {
  return normalizeVue2CompileResult(fromJson(binding.compileToFunctionsVue2(template, dehydrateForNative(options || {}))));
}

function compileSsrVue2(template, options = {}) {
  return normalizeVue2CompileResult(fromJson(binding.compileSsrVue2(template, dehydrateForNative(options || {}))));
}

function generateCodeFrameVue2(source, start = 0, end = start) {
  return binding.generateCodeFrameVue2(String(source || ''), Number(start) || 0, Number(end) || 0);
}

function callVue2Bridge(command, payload = {}) {
  return fromJson(binding.callVue2Bridge(String(command || ''), dehydrateForNative(payload || {})));
}

function dehydrateForNative(value, seen = new WeakSet()) {
  if (typeof value === 'symbol') {
    return String(value).replace(/^Symbol\((.*)\)$/, '$1');
  }
  if (value === undefined || typeof value === 'function') {
    return undefined;
  }
  if (value === null || typeof value !== 'object') {
    return value;
  }
  if (seen.has(value)) {
    return undefined;
  }
  seen.add(value);
  let out;
  if (Array.isArray(value)) {
    out = value.map(item => {
      const dehydrated = dehydrateForNative(item, seen);
      return dehydrated === undefined ? null : dehydrated;
    });
    seen.delete(value);
    return out;
  }
  if (value instanceof Set) {
    out = Array.from(value, item => {
      const dehydrated = dehydrateForNative(item, seen);
      return dehydrated === undefined ? null : dehydrated;
    });
    seen.delete(value);
    return out;
  }
  if (value instanceof Map) {
    out = {};
    for (const [key, item] of value.entries()) {
      const dehydrated = dehydrateForNative(item, seen);
      if (dehydrated !== undefined) {
        out[String(key)] = dehydrated;
      }
    }
    seen.delete(value);
    return out;
  }
  out = {};
  for (const key of Object.keys(value)) {
    const dehydrated = dehydrateForNative(value[key], seen);
    if (dehydrated !== undefined) {
      out[key] = dehydrated;
    }
  }
  seen.delete(value);
  return out;
}

function rewriteDefaultVue27(source, variable, parserPlugins) {
  return binding.rewriteDefaultVue27(String(source || ''), String(variable || ''), parserPlugins || []);
}

function rewriteDefaultVue3(source, variable, parserPlugins) {
  return binding.rewriteDefaultVue3(String(source || ''), String(variable || ''), parserPlugins || []);
}

function baseCompileVue3(source, options = {}) {
  return fromJson(binding.baseCompileVue3(source, dehydrateForNative(options || {})));
}

function baseParseVue3(source, options = {}) {
  return fromJson(binding.baseParseVue3(source, dehydrateForNative(options || {})));
}

function generateVue3Core(ast, options = {}) {
  return fromJson(binding.generateVue3Core(dehydrateForNative(ast || {}), dehydrateForNative(options || {})));
}

function callVue3CoreProjection(command, payload = {}) {
  return fromJson(binding.callVue3CoreProjection(String(command || ''), dehydrateForNative(payload || {})));
}

function callVue3DomProjection(command, payload = {}) {
  return fromJson(binding.callVue3DomProjection(String(command || ''), dehydrateForNative(payload || {})));
}

function compileVue3Dom(source, options = {}) {
  return fromJson(binding.compileVue3Dom(source, dehydrateForNative(options || {})));
}

function parseVue3Dom(source, options = {}) {
  return fromJson(binding.parseVue3Dom(source, dehydrateForNative(options || {})));
}

function compileVue3Ssr(source, options = {}) {
  return fromJson(binding.compileVue3Ssr(source, dehydrateForNative(options || {})));
}

function parseSfc(source, options = {}) {
  return fromJson(binding.parseSfc(source, dehydrateForNative(options || {})));
}

function parseSfcResult(source, options = {}) {
  return fromJson(binding.parseSfcResult(source, dehydrateForNative(options || {})));
}

function parseVue27SfcComponent(source, options = {}) {
  return fromJson(binding.parseVue27SfcComponent(source, dehydrateForNative(options || {})));
}

function compileSfcTemplate(source, options = {}) {
  return fromJson(binding.compileSfcTemplate(source, dehydrateForNative(options || {})));
}

function compileSfcTemplateSource(source, options = {}) {
  return fromJson(binding.compileSfcTemplateSource(source, dehydrateForNative(options || {})));
}

function compileSfcScript(source, options = {}) {
  return fromJson(binding.compileSfcScript(source, dehydrateForNative(options || {})));
}

function compileVue27SfcTemplate(source, options = {}) {
  return fromJson(binding.compileVue27SfcTemplate(source, dehydrateForNative(options || {})));
}

function compileVue27SfcScript(source, options = {}) {
  return fromJson(binding.compileVue27SfcScript(source, dehydrateForNative(options || {})));
}

function compileSfcStyle(source, options = {}) {
  return fromJson(binding.compileSfcStyle(source, dehydrateForNative(resolveStylePreprocessOptions(source, options || {}))));
}

function compile(template, options = {}) {
  return compileVue2(template, options);
}

function compileToFunctions(template, options = {}) {
  return compileToFunctionsVue2(template, options);
}

function baseCompile(source, options = {}) {
  return baseCompileVue3(source, options);
}

function compileDom(source, options = {}) {
  return compileVue3Dom(source, options);
}

function compileSsr(source, options = {}) {
  return compileVue3Ssr(source, options);
}

function parse(source, options = {}) {
  return parseSfc(source, options);
}

function compileTemplate(options) {
  const opts = options || {};
  return compileSfcTemplateSource(String(opts.source || ''), opts);
}

function compileScript(descriptor, options = {}) {
  const source = descriptor && typeof descriptor.source === 'string' ? descriptor.source : '';
  const opts = {
    filename: descriptor && descriptor.filename,
    ...options,
  };
  return compileSfcScript(source, opts);
}

function compileStyle(options) {
  const opts = options || {};
  const source = String(opts.source || '');
  return compileSfcStyle(styleSfcSource(source, opts), resolveStylePreprocessOptions(source, opts));
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

function normalizeVue2CompileResult(result) {
  if (result && Array.isArray(result.static_render_fns) && !Array.isArray(result.staticRenderFns)) {
    result.staticRenderFns = result.static_render_fns;
  }
  return result;
}

function templateSfcSource(source) {
  return /<template(?:\s|>)/i.test(source) ? source : `<template>${source}</template>`;
}

function styleSfcSource(source, options) {
  if (/<style(?:\s|>)/i.test(source)) {
    return source;
  }
  const attrs = [];
  if (options.scoped) {
    attrs.push('scoped');
  }
  if (options.modules || options.module) {
    attrs.push(
      options.module && typeof options.module === 'string'
        ? `module="${escapeAttribute(options.module)}"`
        : 'module'
    );
  }
  const lang = options.lang || options.preprocessLang || options.preprocess_lang;
  if (lang) {
    attrs.push(`lang="${escapeAttribute(lang)}"`);
  }
  return `<style${attrs.length ? ` ${attrs.join(' ')}` : ''}>${source}</style>`;
}

function escapeAttribute(value) {
  return String(value).replace(/&/g, '&amp;').replace(/"/g, '&quot;');
}

function bindingInfo() {
  return {
    source: loadedBinding.source,
    path: loadedBinding.path,
    package: loadedBinding.package,
    platform: process.platform,
    arch: process.arch,
  };
}

module.exports = {
  version: binding.version,
  apiManifest: () => fromJson(binding.apiManifest()),
  bindingInfo,
  compileVue2,
  compileToFunctionsVue2,
  compileSsrVue2,
  generateCodeFrameVue2,
  callVue2Bridge,
  rewriteDefaultVue27,
  rewriteDefaultVue3,
  baseCompileVue3,
  baseParseVue3,
  generateVue3Core,
  callVue3CoreProjection,
  callVue3DomProjection,
  compileVue3Dom,
  parseVue3Dom,
  compileVue3Ssr,
  parseSfc,
  parseSfcResult,
  parseVue27SfcComponent,
  compileSfcTemplate,
  compileSfcTemplateSource,
  compileSfcScript,
  compileVue27SfcTemplate,
  compileVue27SfcScript,
  compileSfcStyle,
  compile,
  compileToFunctions,
  baseCompile,
  compileDom,
  compileSsr,
  parse,
  compileTemplate,
  compileScript,
  compileStyle,
};
