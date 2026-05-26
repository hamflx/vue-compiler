'use strict';

const native = require('@vuec-rs/native');

function compile(source, options) {
  return native.compileVue3Ssr(String(source || ''), options || {});
}

module.exports = {
  compile,
};
