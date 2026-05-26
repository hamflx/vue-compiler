'use strict';

const native = require('@vuec-rs/native');

function compile(template, options) {
  return native.compileVue2(String(template || ''), options || {});
}

function compileToFunctions(template, options, vm) {
  void vm;
  return native.compileToFunctionsVue2(String(template || ''), options || {});
}

const ssrCompile = function compile(template, options) {
  return native.compileSsrVue2(String(template || ''), options || {});
};

const ssrCompileToFunctions = function compileToFunctions(template, options, vm) {
  void vm;
  return native.compileSsrVue2(String(template || ''), options || {});
};

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
