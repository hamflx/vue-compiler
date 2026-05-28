'use strict';

const native = require('@vuec-rs/native');

function compile(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  if (source && typeof source === 'object' && source.type === 0 && Array.isArray(source.children)) {
    const payload = vue3AstCompilePayload(source, options);
    return native.compileVue3Ssr(payload.source, payload.options);
  }
  return native.compileVue3Ssr(String(source || ''), options || {});
}

function vue3AstCompilePayload(ast, options) {
  const source = typeof ast.source === 'string' ? ast.source : '';
  const range = vue3AstChildrenRange(ast, source);
  const template = range ? source.slice(range.start, range.end) : source;
  return {
    source: template,
    options: {
      ...(options || {}),
      __vuecTemplateBaseOffset: range ? range.start : 0,
      __vuecSourceMapSource: source,
      __vuecSourceMapBaseOffset: 0,
    },
  };
}

function vue3AstChildrenRange(ast, source) {
  const children = Array.isArray(ast && ast.children) ? ast.children : [];
  if (!children.length) return { start: 0, end: 0 };
  let start = Infinity;
  let end = -Infinity;
  for (const child of children) {
    const locStart = child && child.loc && child.loc.start && child.loc.start.offset;
    const locEnd = child && child.loc && child.loc.end && child.loc.end.offset;
    if (Number.isFinite(locStart) && Number.isFinite(locEnd) && locEnd >= locStart) {
      start = Math.min(start, locStart);
      end = Math.max(end, locEnd);
    }
  }
  if (!Number.isFinite(start) || end < start) return null;
  return { start, end };
}

module.exports = {
  compile,
};
