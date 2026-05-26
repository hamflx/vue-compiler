'use strict';

const assert = require('assert');
const native = require('./index.js');

assert.strictEqual(typeof native.version(), 'string');

const vue2 = native.compile('<div>{{ msg }}</div>');
assert.match(vue2.render, /with\(this\)\{return _c\(['"]div['"],\[_v\(_s\(msg\)\)\]\)\}/);

const dom = native.compileDom('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
  sourceMap: true,
});
assert.match(dom.code, /export function render/);
assert.match(dom.code, /_toDisplayString\(_ctx\.msg\)/);
assert.ok(dom.map);
assert.strictEqual(dom.map.version, 3);
assert.deepStrictEqual(dom.map.sources, ['anonymous.vue']);
assert.strictEqual(typeof dom.map.mappings, 'string');
assert.deepStrictEqual(dom.map.sourcesContent, ['<div>{{ msg }}</div>']);

const ssr = native.compileSsr('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.match(ssr.code, /export function ssrRender/);
assert.match(ssr.code, /_ssrInterpolate\(_ctx\.msg\)/);

const diagnostic = native.compileDom('<div v-model="x"/>', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.strictEqual(diagnostic.diagnostics[0].severity, 'error');
assert.match(diagnostic.diagnostics[0].message, /v-model/);

const descriptor = native.parse('<template><p/></template>', {
  filename: 'smoke.vue',
});
assert.strictEqual(descriptor.filename, 'smoke.vue');
assert.ok(descriptor.template);

const template = native.compileTemplate({
  source: '<template><div>{{ msg }}</div></template>',
  filename: 'template.vue',
});
assert.match(template.code, /export function render/);

const style = native.compileStyle({
  source: '<style scoped>.a{ color: v-bind(color); }</style>',
  filename: 'style.vue',
  id: 'data-v-smoke',
});
assert.match(style.code, /data-v-smoke/);

const manifest = native.apiManifest();
assert.ok(manifest.exports.includes('compileVue2'));

process.stdout.write(JSON.stringify({
  status: 'pass',
  exports: Object.keys(native).sort(),
}));
