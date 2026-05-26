'use strict';

const native = require('@vuec-rs/native');

function parse(input) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  const opts = options || {};
  if (input && typeof input === 'object') {
    return native.parseSfc(String(input.source || ''), {
      ...opts,
      filename: input.filename || opts.filename,
    });
  }
  return native.parseSfc(String(input || ''), opts);
}

function parseComponent(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  return native.parseSfc(String(source || ''), options || {});
}

function compileTemplate(options) {
  return native.compileTemplate(options || {});
}

function compileScript(descriptor) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  return native.compileScript(descriptor || {}, options || {});
}

function compileStyle(options) {
  return native.compileStyle(options || {});
}

function compileStyleAsync(options) {
  return Promise.resolve(compileStyle(options || {}));
}

function rewriteDefault(source, variable, parserPlugins) {
  return native.rewriteDefaultVue27(String(source || ''), String(variable || ''), parserPlugins || []);
}

function generateCodeFrame(source) {
  const start = arguments.length > 1 ? arguments[1] : undefined;
  const end = arguments.length > 2 ? arguments[2] : undefined;
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
