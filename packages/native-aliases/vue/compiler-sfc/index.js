'use strict';

const native = require('@vuec-rs/native');

function parse(input, options) {
  const opts = options || {};
  if (input && typeof input === 'object') {
    return native.parseSfc(String(input.source || ''), {
      ...opts,
      filename: input.filename || opts.filename,
    });
  }
  return native.parseSfc(String(input || ''), opts);
}

function parseComponent(source, options) {
  return native.parseSfc(String(source || ''), options || {});
}

function unavailable(name) {
  throw new Error(`NAPI vue/compiler-sfc alias ${name} is not implemented in this smoke package yet`);
}

function compileTemplate() {
  return unavailable('compileTemplate');
}

function compileScript() {
  return unavailable('compileScript');
}

function compileStyle() {
  return unavailable('compileStyle');
}

function compileStyleAsync() {
  return Promise.reject(
    new Error('NAPI vue/compiler-sfc alias compileStyleAsync is not implemented in this smoke package yet'),
  );
}

function rewriteDefault() {
  return unavailable('rewriteDefault');
}

function generateCodeFrame(source, start, end) {
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

module.exports = {
  parse,
  parseComponent,
  compileTemplate,
  compileScript,
  compileStyle,
  compileStyleAsync,
  rewriteDefault,
  generateCodeFrame,
};
