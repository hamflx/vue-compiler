'use strict';

const native = require('@vuec-rs/native');

function compile(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  return native.compileVue3Ssr(String(source || ''), options || {});
}

module.exports = {
  compile,
};
