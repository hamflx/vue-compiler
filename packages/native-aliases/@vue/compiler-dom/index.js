'use strict';

const core = require('@vue/compiler-core');
const native = require('@vuec-rs/native');

function compile(source, options) {
  return native.compileVue3Dom(String(source || ''), options || {});
}

function parse() {
  throw new Error('NAPI @vue/compiler-dom alias parse is not implemented in this smoke package yet');
}

module.exports = {
  ...core,
  compile,
  parse,
  baseCompile: core.baseCompile,
  parserOptions: {},
};
