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

function parseComponent(source) {
  return native.parseSfc(String(source || ''), {});
}

function generateCodeFrame(source) {
  return native.generateCodeFrameVue2(String(source || ''), 0, String(source || '').length);
}

module.exports = {
  compile,
  compileToFunctions,
  ssrCompile,
  ssrCompileToFunctions,
  parseComponent,
  generateCodeFrame,
};
