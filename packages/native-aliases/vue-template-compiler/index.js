'use strict';

const native = require('@vuec-rs/native');

function compile(template, options) {
  return native.compileVue2(String(template || ''), options || {});
}

function compileToFunctions(template, options) {
  return native.compileToFunctionsVue2(String(template || ''), options || {});
}

function ssrCompile(template, options) {
  return native.compileSsrVue2(String(template || ''), options || {});
}

function ssrCompileToFunctions(template, options) {
  return native.compileSsrVue2(String(template || ''), options || {});
}

function parseComponent(source, options) {
  return native.parseSfc(String(source || ''), options || {});
}

function generateCodeFrame(source, start, end) {
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

module.exports = {
  compile,
  compileToFunctions,
  ssrCompile,
  ssrCompileToFunctions,
  parseComponent,
  generateCodeFrame,
};
