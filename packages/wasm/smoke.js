import assert from 'node:assert/strict';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { init } from './index.js';

const pkgPath = process.env.VUEC_WASM_PKG && path.isAbsolute(process.env.VUEC_WASM_PKG)
  ? pathToFileURL(process.env.VUEC_WASM_PKG).href
  : process.env.VUEC_WASM_PKG || './pkg/vuec_wasm.js';
const rawMod = await import(pkgPath);
const rawWasm = rawMod && rawMod.default && typeof rawMod.default === 'object' ? rawMod.default : rawMod;
const wasm = await init(pkgPath);

assert.equal(typeof wasm.version(), 'string');

const vue2 = wasm.compile('<div>{{ msg }}</div>');
assert.match(vue2.render, /_s\(msg\)/);

const dom = wasm.compileDom('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
  sourceMap: true,
});
assert.match(dom.code, /export function render/);
assert.match(dom.code, /_toDisplayString\(_ctx\.msg\)/);
assert.equal(dom.map.version, 3);

const diagnostic = wasm.compileDom('<div v-model="x"/>', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.equal(diagnostic.diagnostics[0].severity, 'error');
assert.match(diagnostic.diagnostics[0].message, /v-model/);

const invalidOptions = JSON.parse(rawWasm.compileVue3Dom('<div/>', '{not json'));
assert.equal(invalidOptions.errors[0].code, 'VUEC_WASM_INVALID_OPTIONS_JSON');
assert.equal(invalidOptions.diagnostics[0].severity, 'error');

const ssr = wasm.compileSsr('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.match(ssr.code, /export function ssrRender/);
assert.match(ssr.code, /_ssrInterpolate\(_ctx\.msg\)/);

const descriptor = wasm.parse('<template><p/></template>', {
  filename: 'smoke.vue',
});
assert.equal(descriptor.filename, 'smoke.vue');
assert.ok(descriptor.template);

const template = wasm.compileTemplate({
  source: '<template><div>{{ msg }}</div></template>',
  filename: 'template.vue',
});
assert.match(template.code, /export function render/);

const style = wasm.compileStyle({
  source: '.a{ color: v-bind(color); }',
  filename: 'style.vue',
  id: 'data-v-smoke',
  scoped: true,
});
assert.match(style.code, /data-v-smoke/);

process.stdout.write(JSON.stringify({
  status: 'pass',
  exports: Object.keys(wasm).sort(),
}));
