'use strict';

const native = require('@vuec-rs/native');

function baseCompile(source, options) {
  return native.baseCompileVue3(String(source || ''), options || {});
}

function compile(source, options) {
  return baseCompile(source, options);
}

function unavailable(name) {
  throw new Error(`NAPI @vue/compiler-core alias ${name} is not implemented in this smoke package yet`);
}

function parse() {
  return unavailable('parse');
}

function baseParse() {
  return unavailable('baseParse');
}

function generate() {
  return unavailable('generate');
}

module.exports = {
  baseCompile,
  compile,
  parse,
  baseParse,
  generate,
};
