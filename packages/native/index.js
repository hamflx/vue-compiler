'use strict';

const path = require('path');

const binding = require(path.join(__dirname, 'vuec_napi.node'));

function fromJson(json) {
  return JSON.parse(json);
}

function compileVue2(template, options = {}) {
  return fromJson(binding.compileVue2(template, options));
}

function compileToFunctionsVue2(template, options = {}) {
  return fromJson(binding.compileToFunctionsVue2(template, options));
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
  return compileSfcTemplate(String(opts.source || ''), opts);
}

function compileScript(descriptor, options = {}) {
  const source = descriptor && typeof descriptor.source === 'string' ? descriptor.source : '';
  return compileSfcScript(source, options);
}

function compileStyle(options) {
  const opts = options || {};
  return compileSfcStyle(String(opts.source || ''), opts);
}

module.exports = {
  version: binding.version,
  apiManifest: () => fromJson(binding.apiManifest()),
  compileVue2,
  compileToFunctionsVue2,
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
