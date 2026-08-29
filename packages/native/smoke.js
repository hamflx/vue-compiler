'use strict';

const assert = require('assert');
const native = require('./index.js');
const rawBinding = require('./vuec_napi.node');

assert.strictEqual(typeof native.version(), 'string');
assert.ok(['local', 'platform', 'env'].includes(native.bindingInfo().source));

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

const rawDom = JSON.parse(rawBinding.compileVue3Dom('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
  sourceMap: true,
}));
assert.match(rawDom.code, /export function render/);
assert.ok(rawDom.map);

const sharedOption = { enabled: true };
const rawDomWithSharedOptions = JSON.parse(rawBinding.compileVue3Dom('<div/>', {
  mode: 'module',
  prefixIdentifiers: true,
  first: sharedOption,
  second: sharedOption,
  bytes: new Uint8Array([1, 2]),
  omitted: undefined,
}));
assert.match(rawDomWithSharedOptions.code, /export function render/);

assert.throws(
  () => rawBinding.compileVue3Dom('<div/>', { mode: 'invalid' }),
  (error) => {
    assert.strictEqual(error.code, 'InvalidArg');
    assert.match(error.message, /expected "function" or "module"/);
    return true;
  },
);

const cyclicOptions = {};
cyclicOptions.self = cyclicOptions;
assert.throws(
  () => rawBinding.compileVue3Dom('<div/>', cyclicOptions),
  /circular reference detected/,
);

const deeplyNestedOptions = {};
let nestedCursor = deeplyNestedOptions;
for (let depth = 0; depth < 600; depth += 1) {
  nestedCursor.child = {};
  nestedCursor = nestedCursor.child;
}
assert.throws(
  () => rawBinding.compileVue3Dom('<div/>', deeplyNestedOptions),
  /maximum nesting depth of 512 exceeded/,
);

assert.throws(
  () => rawBinding.compileVue3Dom('<div/>', new Array(100000)),
  /maximum node count of 100000 exceeded/,
);

const rawDomAfterRejectedInput = JSON.parse(rawBinding.compileVue3Dom('<p/>', {
  mode: 'module',
  prefixIdentifiers: true,
}));
assert.match(rawDomAfterRejectedInput.code, /export function render/);

const domAst = native.parseVue3Dom('<div>{{ msg }}</div>');
assert.strictEqual(domAst.children[0].tag, 'div');
assert.deepStrictEqual(
  native.callVue3DomProjection('vue3.dom.transformStyle', {
    node: { props: [{ type: 6, name: 'style', value: { content: 'color: red' } }] },
  }).replacements,
  [{ index: 0, expression: '{"color":"red"}' }],
);

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
assert.deepStrictEqual(style.errors, []);
assert.ok(!style.diagnostics || style.diagnostics.length === 0);

const manifest = native.apiManifest();
assert.ok(manifest.exports.includes('compileVue2'));
assert.ok(manifest.exports.includes('parseSfcResult'));
assert.strictEqual(typeof native.parseSfcResult, 'function');

process.stdout.write(JSON.stringify({
  status: 'pass',
  binding: native.bindingInfo(),
  exports: Object.keys(native).sort(),
}));
