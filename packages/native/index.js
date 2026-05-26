'use strict';

const path = require('path');

const binding = require(path.join(__dirname, 'vuec_napi.node'));

function fromJson(json) {
  return JSON.parse(json);
}

function compileVue2(template, options = {}) {
  return normalizeVue2CompileResult(fromJson(binding.compileVue2(template, options)));
}

function compileToFunctionsVue2(template, options = {}) {
  return normalizeVue2CompileResult(fromJson(binding.compileToFunctionsVue2(template, options)));
}

function compileSsrVue2(template, options = {}) {
  return normalizeVue2CompileResult(fromJson(binding.compileSsrVue2(template, options)));
}

function generateCodeFrameVue2(source, start = 0, end = start) {
  return binding.generateCodeFrameVue2(String(source || ''), Number(start) || 0, Number(end) || 0);
}

function baseCompileVue3(source, options = {}) {
  return fromJson(binding.baseCompileVue3(source, options));
}

function compileVue3Dom(source, options = {}) {
  return fromJson(binding.compileVue3Dom(source, options));
}

function compileVue3Ssr(source, options = {}) {
  return fromJson(binding.compileVue3Ssr(source, options));
}

function parseSfc(source, options = {}) {
  return fromJson(binding.parseSfc(source, options));
}

function compileSfcTemplate(source, options = {}) {
  return fromJson(binding.compileSfcTemplate(source, options));
}

function compileSfcScript(source, options = {}) {
  return fromJson(binding.compileSfcScript(source, options));
}

function compileSfcStyle(source, options = {}) {
  return fromJson(binding.compileSfcStyle(source, options));
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
  return compileSfcTemplate(templateSfcSource(String(opts.source || '')), opts);
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
  return compileSfcStyle(styleSfcSource(String(opts.source || ''), opts), opts);
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
    attrs.push('module');
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

module.exports = {
  version: binding.version,
  apiManifest: () => fromJson(binding.apiManifest()),
  compileVue2,
  compileToFunctionsVue2,
  compileSsrVue2,
  generateCodeFrameVue2,
  baseCompileVue3,
  compileVue3Dom,
  compileVue3Ssr,
  parseSfc,
  compileSfcTemplate,
  compileSfcScript,
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
