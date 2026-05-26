'use strict';

const assert = require('assert');

const vue2 = require('vue-template-compiler');
const vue27Sfc = require('vue/compiler-sfc');
const core = require('@vue/compiler-core');
const dom = require('@vue/compiler-dom');
const ssr = require('@vue/compiler-ssr');
const sfc = require('@vue/compiler-sfc');

const vue2Result = vue2.compile('<div>{{ msg }}</div>');
assert.match(vue2Result.render, /_s\(msg\)/);
assert.ok(Array.isArray(vue2Result.staticRenderFns));

const vue2Functions = vue2.compileToFunctions('<p>{{ ok }}</p>');
assert.match(vue2Functions.render, /_s\(ok\)/);

const frame = vue2.generateCodeFrame('one\ntwo\nthree', 4, 7);
assert.match(frame, /two/);

const vue27Descriptor = vue27Sfc.parse({
  source: '<template><p/></template>',
  filename: 'vue27.vue',
});
assert.strictEqual(vue27Descriptor.filename, 'vue27.vue');
assert.ok(vue27Descriptor.template);

const coreResult = core.baseCompile('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.match(coreResult.code, /export function render/);
assert.match(coreResult.code, /_ctx\.msg/);

const domResult = dom.compile('<input v-model="msg">', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.match(domResult.code, /export function render/);
assert.match(domResult.code, /modelValue/);

const ssrResult = ssr.compile('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true,
});
assert.match(ssrResult.code, /export function ssrRender/);
assert.match(ssrResult.code, /_ssrInterpolate\(_ctx\.msg\)/);

const parsed = sfc.parse('<template><div>{{ msg }}</div></template><script setup>const msg = 1</script>');
assert.ok(parsed.descriptor.template);

const template = sfc.compileTemplate({
  source: '<div>{{ msg }}</div>',
  filename: 'alias.vue',
  id: 'data-v-alias',
});
assert.match(template.code, /export function render/);

const script = sfc.compileScript(parsed.descriptor, {
  id: 'data-v-alias',
});
assert.match(script.content, /setup/);

const style = sfc.compileStyle({
  source: '.a{ color: v-bind(color); }',
  filename: 'alias.vue',
  id: 'data-v-alias',
  scoped: true,
});
assert.match(style.code, /data-v-alias/);

const rewritten = sfc.rewriteDefault('export default { name: "Alias" }', '__default__');
assert.match(rewritten, /const __default__/);

const magic = new sfc.MagicString('abc');
magic.overwrite(1, 2, 'B').append('!');
assert.strictEqual(magic.toString(), 'aBc!');

const identifiers = sfc.extractIdentifiers({
  type: 'ObjectPattern',
  properties: [
    {
      type: 'ObjectProperty',
      key: { type: 'Identifier', name: 'source' },
      value: { type: 'Identifier', name: 'target' },
    },
  ],
});
assert.ok(identifiers.some(identifier => identifier.name === 'target'));

assert.deepStrictEqual(
  sfc.inferRuntimeType({}, {
    type: 'TSUnionType',
    types: [{ type: 'TSStringKeyword' }, { type: 'TSNumberKeyword' }],
  }),
  ['String', 'Number'],
);

process.stdout.write(JSON.stringify({
  status: 'pass',
  packages: [
    'vue-template-compiler',
    'vue/compiler-sfc',
    '@vue/compiler-core',
    '@vue/compiler-dom',
    '@vue/compiler-ssr',
    '@vue/compiler-sfc',
  ],
}));
