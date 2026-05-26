'use strict';

const native = require('@vuec-rs/native');

function parse(source, options) {
  return { descriptor: native.parseSfc(String(source || ''), options || {}), errors: [] };
}

function compileTemplate(options) {
  return native.compileTemplate(options || {});
}

function compileScript(descriptor, options) {
  return native.compileScript(descriptor, options || {});
}

function compileStyle(options) {
  return native.compileStyle(options || {});
}

function compileStyleAsync(options) {
  return Promise.resolve(compileStyle(options));
}

function generateCodeFrame(source, start, end) {
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

module.exports = {
  parse,
  compileTemplate,
  compileScript,
  compileStyle,
  compileStyleAsync,
  generateCodeFrame,
  version: native.version(),
};
