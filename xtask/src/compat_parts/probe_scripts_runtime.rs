const RUNNER_STARTUP_PROBE_SCRIPT: &str = r#"
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const nodeModules = process.env.VUEC_RUNNER_NODE_MODULES;
const request = process.env.VUEC_RUNNER_PROBE_REQUEST;
const rootRequire = createRequire(path.join(nodeModules, '__vuec_runner_probe__.cjs'));

const resolved = rootRequire.resolve(request);
await import(pathToFileURL(resolved).href);
"#;

const API_PROBE_SCRIPT: &str = r#"
const fs = require('fs');
const path = require('path');
const { createRequire } = require('module');

const root = process.env.VUEC_API_PROBE_ROOT;
const packageName = process.env.VUEC_API_PROBE_PACKAGE;
const request = process.env.VUEC_API_PROBE_REQUEST;
const rootRequire = createRequire(path.join(root, 'package.json'));

function normalizePath(file) {
  if (!file) return null;
  const relative = path.relative(root, file);
  if (relative && !relative.startsWith('..') && !path.isAbsolute(relative)) {
    return '<probe-root>/' + relative.replace(/\\/g, '/');
  }
  return file.replace(/\\/g, '/');
}

function normalizeMessage(message) {
  if (!message) return null;
  const normalizedRoot = root.replace(/\\/g, '/');
  return String(message)
    .replaceAll(root, '<probe-root>')
    .replaceAll(normalizedRoot, '<probe-root>')
    .replace(/\\/g, '/');
}

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (_) {
    return null;
  }
}

function describeExport(value) {
  const kind = typeof value;
  const tag = Object.prototype.toString.call(value);
  const detail = {
    kind,
    tag,
    name: value && value.name ? String(value.name) : null,
    function_arity: kind === 'function' ? value.length : null,
    is_async_function: kind === 'function' ? value.constructor && value.constructor.name === 'AsyncFunction' : null,
    is_class_like: kind === 'function' ? /^class\s/.test(Function.prototype.toString.call(value)) : null,
    own_property_names: []
  };
  try {
    detail.own_property_names = Object.getOwnPropertyNames(value).sort();
  } catch (_) {
    detail.own_property_names = [];
  }
  return detail;
}

function resolvePackageJson() {
  try {
    return rootRequire.resolve(path.join(packageName, 'package.json'));
  } catch (_) {
    try {
      const resolved = rootRequire.resolve(request);
      let current = path.dirname(resolved);
      while (current && current !== path.dirname(current)) {
        const candidate = path.join(current, 'package.json');
        if (fs.existsSync(candidate)) return candidate;
        current = path.dirname(current);
      }
    } catch (_) {}
  }
  return null;
}

function resolveTypesPath(packageJsonPath, packageJson) {
  if (!packageJsonPath || !packageJson) return { packageTypes: null, resolved: null };
  const packageRoot = path.dirname(packageJsonPath);
  if (packageJson.exports && request.startsWith(packageName + '/')) {
    const subpath = './' + request.slice(packageName.length + 1);
    const exportRecord = packageJson.exports[subpath];
    if (exportRecord && typeof exportRecord === 'object' && typeof exportRecord.types === 'string') {
      const resolved = path.resolve(packageRoot, exportRecord.types);
      return { packageTypes: exportRecord.types, resolved };
    }
  }
  if (typeof packageJson.types === 'string') {
    return {
      packageTypes: packageJson.types,
      resolved: path.resolve(packageRoot, packageJson.types)
    };
  }
  return { packageTypes: null, resolved: null };
}

const packageJsonPath = resolvePackageJson();
const packageJson = packageJsonPath ? readJson(packageJsonPath) : null;
const typesInfo = resolveTypesPath(packageJsonPath, packageJson);
const out = {
  package_version: packageJson && packageJson.version ? String(packageJson.version) : null,
  exports: [],
  export_details: {},
  require: {
    request,
    success: false,
    resolved: null,
    error_name: null,
    error_code: null,
    error_message: null
  },
  types: {
    package_types: typesInfo.packageTypes,
    resolved: normalizePath(typesInfo.resolved),
    exists: typesInfo.resolved ? fs.existsSync(typesInfo.resolved) : false
  }
};

try {
  out.require.resolved = normalizePath(rootRequire.resolve(request));
  const api = rootRequire(request);
  out.require.success = true;
  out.exports = Object.keys(api).sort();
  for (const key of out.exports) {
    out.export_details[key] = describeExport(api[key]);
  }
} catch (error) {
  out.require.error_name = error && error.name ? String(error.name) : null;
  out.require.error_code = error && error.code ? String(error.code) : null;
  out.require.error_message = normalizeMessage(error && error.message ? error.message : error);
}

process.stdout.write(JSON.stringify(out));
"#;

const ALIAS_RUNTIME_FRAGMENT_CALLBACK_PROVENANCE_MARKER: &str =
    "vuec-runtime-fragment: callback-boundary/provenance-runtime";
const ALIAS_RUNTIME_FRAGMENT_SEMANTIC_JS_MARKER: &str =
    "vuec-runtime-fragment: semantic-js-shim/vue3-core-runtime";
const ALIAS_RUNTIME_FRAGMENT_BRIDGE_SHAPE_MARKER: &str =
    "vuec-runtime-fragment: bridge-shape-adapter/node-bridge-call";
const ALIAS_RUNTIME_FRAGMENT_SUITE_HELPER_MARKER: &str =
    "vuec-runtime-fragment: suite-helper/runtime-entrypoint";
const ALIAS_RUNTIME_FRAGMENT_PACKAGE_API_MARKER: &str =
    "vuec-runtime-fragment: package-api-adapter/public-package-shapes";
const ALIAS_RUNTIME_FRAGMENT_CALLBACK_MATERIALIZATION_MARKER: &str =
    "vuec-runtime-fragment: callback-boundary/js-callback-materialization";

#[derive(Clone, Copy, Debug)]
struct AliasRuntimeFragmentSpec {
    order: u32,
    name: &'static str,
    role: &'static str,
    source: &'static str,
    source_anchor: &'static str,
    execution_path: &'static str,
    migration_note: Option<&'static str>,
}

const ALIAS_RUNTIME_FRAGMENT_SPECS: &[AliasRuntimeFragmentSpec] = &[
    AliasRuntimeFragmentSpec {
        order: 10,
        name: "provenance-runtime",
        role: "callback-boundary",
        source: "generated-alias-runtime",
        source_anchor: ALIAS_RUNTIME_FRAGMENT_CALLBACK_PROVENANCE_MARKER,
        execution_path: "mixed-js-callback-boundary",
        migration_note: None,
    },
    AliasRuntimeFragmentSpec {
        order: 20,
        name: "vue3-core-runtime",
        role: "semantic-js-shim",
        source: "generated-alias-runtime",
        source_anchor: ALIAS_RUNTIME_FRAGMENT_SEMANTIC_JS_MARKER,
        execution_path: "shim-backed-semantic-js",
        migration_note: Some(
            "Migrate createTransformContext, transform traversal, baseCompile fallback, transformElement prop/directive materialization, and related helper semantics into Rust compiler-core projections before counting these paths as pure Rust.",
        ),
    },
    AliasRuntimeFragmentSpec {
        order: 30,
        name: "node-bridge-call",
        role: "bridge-shape-adapter",
        source: "generated-alias-runtime",
        source_anchor: ALIAS_RUNTIME_FRAGMENT_BRIDGE_SHAPE_MARKER,
        execution_path: "rust-bridge-shape-adapter",
        migration_note: None,
    },
    AliasRuntimeFragmentSpec {
        order: 40,
        name: "runtime-entrypoint",
        role: "suite-helper",
        source: "generated-alias-runtime",
        source_anchor: ALIAS_RUNTIME_FRAGMENT_SUITE_HELPER_MARKER,
        execution_path: "hybrid-js-adapter-rust-projection",
        migration_note: None,
    },
    AliasRuntimeFragmentSpec {
        order: 50,
        name: "public-package-shapes",
        role: "package-api-adapter",
        source: "generated-alias-runtime",
        source_anchor: ALIAS_RUNTIME_FRAGMENT_PACKAGE_API_MARKER,
        execution_path: "rust-bridge-shape-adapter",
        migration_note: None,
    },
    AliasRuntimeFragmentSpec {
        order: 60,
        name: "js-callback-materialization",
        role: "callback-boundary",
        source: "generated-alias-runtime",
        source_anchor: ALIAS_RUNTIME_FRAGMENT_CALLBACK_MATERIALIZATION_MARKER,
        execution_path: "mixed-js-callback-boundary",
        migration_note: None,
    },
];

const ALIAS_RUNTIME_JS: &str = include_str!("alias_runtime.js");

const VUE2_PROJECT_CORPUS_PROBE_SCRIPT: &str = r#"
const fs = require('fs');
const path = require('path');
const { createRequire } = require('module');

const officialRoot = process.env.VUEC_PROJECT_OFFICIAL_ROOT;
const rustRoot = process.env.VUEC_PROJECT_RUST_ROOT;
const projectRoot = process.env.VUEC_PROJECT_ROOT;
const files = JSON.parse(process.env.VUEC_PROJECT_FILES || '[]');

const officialRequire = createRequire(path.join(officialRoot, 'package.json'));
const rustRequire = createRequire(path.join(rustRoot, 'node_modules', '.vuec-probe.js'));
const officialTemplate = officialRequire('vue-template-compiler');
const rustTemplate = rustRequire('vue-template-compiler');
const officialSfc = officialRequire('vue/compiler-sfc');
const rustSfc = rustRequire('vue/compiler-sfc');

function normalize(value) {
  if (value == null) return value;
  if (typeof value === 'function') return `[Function:${value.name || 'anonymous'}]`;
  if (value instanceof Set) return Array.from(value).sort().map(normalize);
  if (Array.isArray(value)) return value.map(normalize);
  if (typeof value === 'object') {
    const out = {};
    for (const key of Object.keys(value).sort()) {
      if (key === 'ast' || key === 'map' || key === 'rawResult') continue;
      if (key.startsWith('__vuec')) continue;
      out[key] = normalize(value[key]);
    }
    return out;
  }
  return value;
}

function normalizeCompile(result) {
  const out = normalize(result) || {};
  if (!Array.isArray(out.staticRenderFns)) out.staticRenderFns = [];
  return out;
}

function normalizeDescriptor(result) {
  const descriptor = result && result.descriptor ? result.descriptor : result;
  if (!descriptor || typeof descriptor !== 'object') return descriptor;
  return normalize({
    template: descriptor.template && {
      content: descriptor.template.content,
      attrs: descriptor.template.attrs,
      lang: descriptor.template.lang,
      src: descriptor.template.src,
    },
    script: descriptor.script && {
      content: descriptor.script.content,
      attrs: descriptor.script.attrs,
      lang: descriptor.script.lang,
      src: descriptor.script.src,
    },
    scriptSetup: descriptor.scriptSetup && {
      content: descriptor.scriptSetup.content,
      attrs: descriptor.scriptSetup.attrs,
      lang: descriptor.scriptSetup.lang,
      src: descriptor.scriptSetup.src,
    },
    styles: Array.isArray(descriptor.styles)
      ? descriptor.styles.map(style => ({
          content: style.content,
          attrs: style.attrs,
          lang: style.lang,
          scoped: style.scoped,
          module: style.module,
          src: style.src,
        }))
      : [],
    customBlocks: Array.isArray(descriptor.customBlocks)
      ? descriptor.customBlocks.map(block => ({
          type: block.type,
          content: block.content,
          attrs: block.attrs,
        }))
      : [],
    errors: descriptor.errors || (result && result.errors) || [],
  });
}

function firstTemplate(result) {
  const descriptor = result && result.descriptor ? result.descriptor : result;
  return descriptor && descriptor.template && descriptor.template.content
    ? descriptor.template.content
    : null;
}

function capture(fn) {
  try {
    return { ok: true, value: normalize(fn()) };
  } catch (error) {
    return {
      ok: false,
      error: {
        name: error && error.name ? String(error.name) : null,
        message: error && error.message ? String(error.message) : String(error),
      },
    };
  }
}

function compareMode(mode, official, rust) {
  if (JSON.stringify(official) === JSON.stringify(rust)) return null;
  return {
    mode,
    official,
    rust,
  };
}

const reports = [];
let templateFiles = 0;
let pass = 0;
let fail = 0;
let modes = 0;

for (const file of files) {
  const absolute = path.join(projectRoot, file);
  const source = fs.readFileSync(absolute, 'utf8');
  const parsedOfficial = capture(() => normalizeDescriptor(officialSfc.parse({ source, filename: file })));
  const parsedRust = capture(() => normalizeDescriptor(rustSfc.parse({ source, filename: file })));
  const diffs = [];
  let fileModes = 0;

  for (const diff of [
    compareMode('sfc.parse', parsedOfficial, parsedRust),
  ]) {
    fileModes++;
    modes++;
    if (diff) {
      fail++;
      diffs.push(diff);
    } else {
      pass++;
    }
  }

  const template = firstTemplate(officialSfc.parse({ source, filename: file }));
  if (template != null && template.trim() !== '') {
    templateFiles++;
    const compileOptions = { outputSourceRange: true, comments: true };
    const officialCompile = capture(() => normalizeCompile(officialTemplate.compile(template, compileOptions)));
    const rustCompile = capture(() => normalizeCompile(rustTemplate.compile(template, compileOptions)));
    const officialSfcTemplate = capture(() => normalize(officialSfc.compileTemplate({
      source: template,
      filename: file,
      id: 'data-v-project',
      scoped: source.includes('<style scoped'),
    })));
    const rustSfcTemplate = capture(() => normalize(rustSfc.compileTemplate({
      source: template,
      filename: file,
      id: 'data-v-project',
      scoped: source.includes('<style scoped'),
    })));

    for (const diff of [
      compareMode('vue-template-compiler.compile', officialCompile, rustCompile),
      compareMode('vue/compiler-sfc.compileTemplate', officialSfcTemplate, rustSfcTemplate),
    ]) {
      fileModes++;
      modes++;
      if (diff) {
        fail++;
        diffs.push(diff);
      } else {
        pass++;
      }
    }
  }

  reports.push({ path: file, modes: fileModes, diffs });
}

process.stdout.write(JSON.stringify({
  status: fail === 0 ? 'pass' : 'fail',
  counts: {
    files: files.length,
    template_files: templateFiles,
    modes,
    pass,
    fail,
  },
  files: reports,
}));
"#;

const OUTPUT_CONTRACT_PROBE_SCRIPT: &str = r#"
const path = require('path');
const { createRequire } = require('module');

const officialRoot = process.env.VUEC_OUTPUT_OFFICIAL_ROOT;
const rustRoot = process.env.VUEC_OUTPUT_RUST_ROOT;
const request = process.env.VUEC_OUTPUT_REQUEST;
const kind = process.env.VUEC_OUTPUT_KIND;
const fixture = process.env.VUEC_OUTPUT_FIXTURE || '';
const versionLine = process.env.VUEC_OUTPUT_VERSION_LINE || '';
const entry = process.env.VUEC_OUTPUT_ENTRY || '';

const officialRequire = createRequire(path.join(officialRoot, 'package.json'));
const rustRequire = createRequire(path.join(rustRoot, 'package.json'));

function load(rootRequire) {
  return rootRequire(request);
}

function isVue27Sfc() {
  return versionLine === 'vue2_7' && entry === 'vue/compiler-sfc';
}

function extractStyleSource(source) {
  const match = String(source).match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  return match ? match[1] : source;
}

function extractTemplateSource(source) {
  const match = String(source).match(/<template[^>]*>([\s\S]*?)<\/template>/i);
  return match ? match[1] : source;
}

function parseSfc(api) {
  return isVue27Sfc()
    ? api.parse({ source: fixture, filename: 'contract.vue' })
    : api.parse(fixture, { filename: 'contract.vue' });
}

function invoke(api) {
  switch (kind) {
    case 'vue2-template': {
      const compile = api.compile(fixture, { outputSourceRange: true, comments: true });
      const functions = api.compileToFunctions(fixture, {}, {});
      return { compile, compileToFunctions: functions };
    }
    case 'sfc': {
      const parsed = parseSfc(api);
      const descriptor = parsed && parsed.descriptor ? parsed.descriptor : parsed;
      const templateSource = descriptor && descriptor.template && descriptor.template.content ? descriptor.template.content : '';
      const styleSource = descriptor && descriptor.styles && descriptor.styles[0] && descriptor.styles[0].content ? descriptor.styles[0].content : '';
      const template = api.compileTemplate({
        source: isVue27Sfc() ? extractTemplateSource(fixture) : templateSource,
        filename: 'contract.vue',
        id: 'data-v-contract',
        scoped: true
      });
      const script = api.compileScript(descriptor, { id: 'data-v-contract' });
      const style = api.compileStyle({
        source: isVue27Sfc() ? extractStyleSource(fixture) : styleSource,
        filename: 'contract.vue',
        id: 'data-v-contract',
        scoped: true
      });
      return { parse: parsed, compileTemplate: template, compileScript: script, compileStyle: style };
    }
    case 'vue3-core':
      return {
        baseCompile: api.baseCompile(fixture, { mode: 'function' }),
        baseParse: api.baseParse(fixture, {})
      };
    case 'vue3-dom':
      return {
        compile: api.compile(fixture, { mode: 'function' }),
        parse: api.parse(fixture, {})
      };
    case 'vue3-ssr':
      return { compile: api.compile(fixture, {}) };
    default:
      throw new Error(`unknown output contract kind ${kind}`);
  }
}

function capture(side, fn) {
  try {
    return { side, ok: true, value: normalize(fn()) };
  } catch (error) {
    return {
      side,
      ok: false,
      error: {
        name: error && error.name ? String(error.name) : null,
        code: error && error.code ? String(error.code) : null,
        message: error && error.message ? normalizeMessage(error.message) : String(error)
      }
    };
  }
}

function normalizeMessage(message) {
  return String(message)
    .replaceAll(officialRoot.replace(/\\/g, '/'), '<official-root>')
    .replaceAll(rustRoot.replace(/\\/g, '/'), '<rust-root>')
    .replaceAll(officialRoot, '<official-root>')
    .replaceAll(rustRoot, '<rust-root>')
    .replace(/\\/g, '/');
}

function normalize(value, seen = new WeakSet()) {
  if (value === undefined) return { __type: 'undefined' };
  if (value === null) return null;
  if (typeof value === 'function') {
    return { __type: 'function', name: value.name, length: value.length };
  }
  if (typeof value === 'symbol') {
    return { __type: 'symbol', description: value.description || null };
  }
  if (typeof value !== 'object') return value;
  if (seen.has(value)) return { __type: 'cycle' };
  seen.add(value);
  if (Array.isArray(value)) return value.map(item => normalize(item, seen));
  if (value instanceof Set) {
    return Array.from(value).map(item => normalize(item, seen));
  }
  if (value instanceof Map) {
    return Array.from(value.entries()).map(([key, item]) => [normalize(key, seen), normalize(item, seen)]);
  }
  const out = {};
  for (const key of Object.keys(value).sort()) {
    if (key === 'ast' || key === 'element_ast' || key === 'source' || key === 'source_file') continue;
    out[key] = normalize(value[key], seen);
  }
  return out;
}

function objectShape(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  return Object.keys(value).sort();
}

function codeFields(value, prefix = '') {
  const out = {};
  collectCodeFields(value, prefix, out, new WeakSet());
  return out;
}

function collectCodeFields(value, prefix, out, seen) {
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectCodeFields(item, `${prefix}[${index}]`, out, seen));
    return;
  }
  for (const key of Object.keys(value).sort()) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (['code', 'render', 'ssrRender'].includes(key) && typeof value[key] === 'string') {
      out[next] = value[key];
    } else if (key === 'staticRenderFns' && Array.isArray(value[key])) {
      out[next] = value[key];
    } else {
      collectCodeFields(value[key], next, out, seen);
    }
  }
}

function diagnosticFields(value, prefix = '') {
  const out = {};
  collectDiagnosticFields(value, prefix, out, new WeakSet());
  return out;
}

function collectDiagnosticFields(value, prefix, out, seen) {
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectDiagnosticFields(item, `${prefix}[${index}]`, out, seen));
    return;
  }
  for (const key of Object.keys(value).sort()) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (['errors', 'warnings', 'tips', 'diagnostics'].includes(key) && Array.isArray(value[key])) {
      out[next] = normalize(value[key]);
    } else {
      collectDiagnosticFields(value[key], next, out, seen);
    }
  }
}

function sourceMapFields(value, prefix = '') {
  const out = {};
  collectSourceMapFields(value, prefix, out, new WeakSet());
  return out;
}

function collectSourceMapFields(value, prefix, out, seen) {
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectSourceMapFields(item, `${prefix}[${index}]`, out, seen));
    return;
  }
  for (const key of Object.keys(value).sort()) {
    const next = prefix ? `${prefix}.${key}` : key;
    if ((key === 'map' || key === 'sourceMap') && value[key] != null) {
      out[next] = normalize(value[key]);
    } else {
      collectSourceMapFields(value[key], next, out, seen);
    }
  }
}

function compareJson(mode, official, rust, extractor) {
  const officialValue = official.ok ? extractor(official.value) : official.error;
  const rustValue = rust.ok ? extractor(rust.value) : rust.error;
  const equal = JSON.stringify(officialValue) === JSON.stringify(rustValue);
  return {
    mode,
    status: equal ? 'pass' : 'fail',
    official: officialValue,
    rust: rustValue
  };
}

const official = capture('official', () => invoke(load(officialRequire)));
const rust = capture('rust', () => invoke(load(rustRequire)));
function topLevelShape(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  if ('descriptor' in value && 'errors' in value) {
    return objectShape(value);
  }
  if ('ast' in value && 'code' in value && 'map' in value) {
    return objectShape(value);
  }
  return objectShape(value);
}

function topLevelCodeFields(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  if (value.code && typeof value.code === 'string') {
    out.code = value.code;
  }
  if (value.render && typeof value.render === 'string') {
    out.render = value.render;
  }
  if (value.ssrRender && typeof value.ssrRender === 'string') {
    out.ssrRender = value.ssrRender;
  }
  return out;
}

function topLevelSourceMap(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  if (value.map !== undefined) {
    out.map = normalize(value.map);
  }
  if (value.rawResult !== undefined) {
    out.rawResult = { keys: Object.keys(value.rawResult).sort() };
  }
  return out;
}

function topLevelRuntime(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  out.runtime = normalize(executeRuntime(value));
  return out;
}

function executeRuntime(value) {
  if (kind === 'vue2-template') {
    return executeVue2Runtime(value.compile || value);
  }
  if (kind === 'sfc' && isVue27Sfc()) {
    return executeVue2Runtime(value.compileTemplate || value);
  }
  if (kind === 'vue3-ssr') {
    return executeVue3SsrRuntime(value.compile || value);
  }
  if (kind === 'vue3-core') {
    return executeVue3Runtime(value.baseCompile || value);
  }
  if (kind === 'vue3-dom') {
    return executeVue3Runtime(value.compile || value);
  }
  if (kind === 'sfc') {
    return executeVue3Runtime(value.compileTemplate || value);
  }
  throw new Error(`unsupported runtime kind ${kind}`);
}

function pickCodeSource(entry, keys) {
  if (!entry || typeof entry !== 'object') {
    return null;
  }
  for (const key of keys) {
    const value = entry[key];
    if (typeof value === 'string' && value.trim()) {
      return value;
    }
  }
  return null;
}

function runtimeFixtureContext() {
  return {
    msg: 'hello',
    a: 'alpha',
    b: 'beta',
    c: 'gamma',
    d: 'delta',
    color: 'rebeccapurple',
    checked: true,
    value: 'value',
    item: { id: 1, name: 'one', uid: 1, ok: true },
    items: [
      { id: 1, name: 'one', uid: 1, ok: true },
      { id: 2, name: 'two', uid: 2, ok: false },
    ],
    list: [
      { id: 1, name: 'one', uid: 1, ok: true },
      { id: 2, name: 'two', uid: 2, ok: true },
    ],
    save: () => 'saved',
    $slots: {},
    _ssrInterpolate: (value) => ssrInterpolate(value),
    _ssrRenderAttrs: (value) => ssrRenderAttrs(value),
    _mergeProps: (...args) => Object.assign({}, ...args),
  };
}

function executeVue2Runtime(entry) {
  if (entry && typeof entry.render === 'function') {
    return executeVue2Render(entry.render, entry.staticRenderFns || []);
  }
  const source = pickCodeSource(entry, ['code', 'render']);
  return executeVue2RenderSource(source);
}

function executeVue2RenderSource(source) {
  if (!source) throw new Error('missing Vue 2 render source');
  const compiled = new Function(`var render = function render() { ${source} };\nreturn { render, staticRenderFns: [] };`)();
  return executeVue2Render(compiled.render, compiled.staticRenderFns || []);
}

function executeVue2Render(render, staticRenderFns) {
  if (typeof render !== 'function') {
    throw new Error('Vue 2 render did not evaluate to a function');
  }
  const context = createVue2RuntimeContext(staticRenderFns);
  return render.call(context);
}

function createVue2RuntimeContext(staticRenderFns) {
  const context = runtimeFixtureContext();
  context._self = context;
  context.$options = {
    staticRenderFns,
    filters: {},
  };
  context.$slots = {};
  context.$scopedSlots = {};
  context._c = function(tag, data, children) {
    return {
      kind: 'vue2-element',
      tag,
      data: normalize(data),
      children: normalize(children),
    };
  };
  context._v = function(text) {
    return {
      kind: 'vue2-text',
      text: String(text),
    };
  };
  context._s = function(value) {
    if (value == null) return '';
    if (typeof value === 'object') return JSON.stringify(normalize(value));
    return String(value);
  };
  context._l = function(list, fn) {
    const source = Array.isArray(list) ? list : list == null ? [] : [list];
    return source.map((item, index) => normalize(fn.call(context, item, index)));
  };
  context._e = function() {
    return {
      kind: 'vue2-comment',
      text: '',
    };
  };
  context._m = function(index) {
    const renderFn = staticRenderFns[index];
    return typeof renderFn === 'function' ? normalize(renderFn.call(context)) : null;
  };
  context._f = function(name) {
    return context.$options.filters[name] || ((value) => value);
  };
  context._o = function(value) {
    return value;
  };
  context._n = function(value) {
    return value;
  };
  context._t = function(name, fallback) {
    return typeof fallback === 'function' ? fallback() : fallback;
  };
  context._u = function(value) {
    return value;
  };
  context._g = function(data, value) {
    return Object.assign({}, data, value);
  };
  context._d = function(list, value) {
    return value;
  };
  context._b = function(data, tag, value) {
    return Object.assign({}, data, value);
  };
  context._k = function() {
    return false;
  };
  return context;
}

function executeVue3Runtime(entry) {
  if (entry && typeof entry.render === 'function') {
    return entry.render(runtimeFixtureContext(), []);
  }
  const source = pickCodeSource(entry, ['code', 'render']);
  return instantiateVue3Render(source)(runtimeFixtureContext(), []);
}

function instantiateVue3Render(source) {
  if (!source) throw new Error('missing Vue 3 render source');
  const transformed = transformVue3ModuleSource(source);
  const factory = new Function(
    'Vue',
    'require',
    '__ctx',
    `with (__ctx) { ${transformed}\nreturn typeof render === 'function' ? render : undefined; }`
  );
  const render = factory(createVue3Runtime(), createVue3SsrRequire(), runtimeFixtureContext());
  if (typeof render !== 'function') {
    throw new Error('Vue 3 render did not evaluate to a function');
  }
  return render;
}

function createVue3Runtime() {
  return {
    mergeProps: (...args) => Object.assign({}, ...args),
    openBlock: () => null,
    createElementVNode: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createElementBlock: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createVNode: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createBlock: (type, props, children) => ({
      kind: 'vue3-node',
      type,
      props: normalize(props),
      children: normalize(children),
    }),
    createTextVNode: (text) => ({
      kind: 'vue3-text',
      text: String(text),
    }),
    createCommentVNode: (text) => ({
      kind: 'vue3-comment',
      text: String(text),
    }),
    toDisplayString: (value) => (value == null ? '' : String(value)),
    renderSlot: (slots, name, props, fallback) => {
      const slot = slots && slots[name];
      if (typeof slot === 'function') {
        return slot(props || {});
      }
      if (typeof fallback === 'function') {
        return fallback();
      }
      return {
        kind: 'vue3-slot',
        name,
        props: normalize(props),
      };
    },
    resolveComponent: (name) => name,
    withCtx: (fn) => fn,
    Fragment: 'Fragment',
    Text: 'Text',
    Comment: 'Comment',
  };
}

function instantiateVue3SsrRender(source) {
  if (!source) throw new Error('missing Vue 3 SSR source');
  const transformed = transformVue3ModuleSource(source);
  const factory = new Function(
    'require',
    '__ctx',
    `with (__ctx) { ${transformed}\nreturn typeof ssrRender === 'function' ? ssrRender : undefined; }`
  );
  const ssrRender = factory(createVue3SsrRequire(), runtimeFixtureContext());
  if (typeof ssrRender !== 'function') {
    throw new Error('Vue 3 SSR render did not evaluate to a function');
  }
  return ssrRender;
}

function executeVue3SsrRuntime(entry) {
  const ssrRender = entry && typeof entry.ssrRender === 'function'
    ? entry.ssrRender
    : instantiateVue3SsrRender(pickCodeSource(entry, ['code', 'ssrRender']));
  const chunks = [];
  const push = (chunk) => {
    if (chunk == null) return;
    chunks.push(String(chunk));
  };
  ssrRender(runtimeFixtureContext(), push, null, {});
  return chunks.join('');
}

function createVue3SsrRequire() {
  const serverRenderer = {
    ssrRenderAttrs: (props) => ssrRenderAttrs(props),
    ssrInterpolate: (value) => ssrInterpolate(value),
    ssrRenderInterpolate: (value) => ssrInterpolate(value),
    ssrRenderSlot: (slots, name, props, fallbackRenderFn, push) => {
      const slot = slots && slots[name];
      if (typeof slot === 'function') {
        const result = slot(props || {});
        if (Array.isArray(result)) {
          for (const item of result) {
            push(String(item));
          }
        } else if (result != null) {
          push(String(result));
        }
      } else if (typeof fallbackRenderFn === 'function') {
        fallbackRenderFn();
      }
    },
    ssrRenderList: (source, renderItem) => {
      const list = Array.isArray(source) ? source : source == null ? [] : [source];
      return list.map((item, index) => renderItem(item, index));
    },
    ssrRenderComponent: () => '',
    ssrRenderTeleport: () => {},
    ssrRenderSuspense: (push, slots) => {
      if (slots && typeof slots.default === 'function') {
        slots.default();
      }
      return Promise.resolve();
    },
    ssrRenderDynamicModel: () => '',
    ssrRenderAttr: (key, value) => ssrRenderAttrs({ [key]: value }),
    ssrRenderClass: (raw) => (raw == null ? '' : ssrEscape(String(raw))),
    ssrRenderStyle: (raw) => (raw == null ? '' : ssrEscape(String(raw))),
  };
  return (id) => {
    if (id === 'vue') {
      return {
        mergeProps: (...args) => Object.assign({}, ...args),
      };
    }
    if (id === 'vue/server-renderer') {
      return serverRenderer;
    }
    throw new Error(`unsupported runtime require ${id}`);
  };
}

function transformVue3ModuleSource(source) {
  return String(source)
    .replace(/import\s+\{([\s\S]*?)\}\s+from\s+["']vue["'];?\s*/g, (_, specifiers) => {
      return `const { ${specifiers.replace(/\s+as\s+/g, ': ')} } = Vue;\n`;
    })
    .replace(/import\s+\{([\s\S]*?)\}\s+from\s+["']vue\/server-renderer["'];?\s*/g, (_, specifiers) => {
      return `const { ${specifiers.replace(/\s+as\s+/g, ': ')} } = require("vue/server-renderer");\n`;
    })
    .replace(/export\s+function\s+(render|ssrRender)/g, 'function $1');
}

function ssrRenderAttrs(props) {
  if (!props || typeof props !== 'object') return '';
  const attrs = [];
  for (const key of Object.keys(props).sort()) {
    const value = props[key];
    if (value == null || value === false) continue;
    if (value === true) {
      attrs.push(` ${key}`);
    } else {
      attrs.push(` ${key}="${ssrEscape(String(value))}"`);
    }
  }
  return attrs.join('');
}

function ssrInterpolate(value) {
  return ssrEscape(value == null ? '' : String(value));
}

function ssrEscape(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function topLevelDiagnostics(value) {
  const out = {};
  if (!value || typeof value !== 'object') return out;
  if (Array.isArray(value.errors)) out.errors = normalize(value.errors);
  if (Array.isArray(value.tips)) out.tips = normalize(value.tips);
  if (Array.isArray(value.warnings)) out.warnings = normalize(value.warnings);
  if (Array.isArray(value.diagnostics)) out.diagnostics = normalize(value.diagnostics);
  return out;
}

const checks = [
  compareJson('schema-parity', official, rust, value => topLevelShape(value)),
  compareJson('exact-js-output', official, rust, value => topLevelCodeFields(value)),
  compareJson('diagnostic-parity', official, rust, value => topLevelDiagnostics(value)),
  compareJson('source-map-parity', official, rust, value => topLevelSourceMap(value)),
  compareJson('runtime-parity', official, rust, value => topLevelRuntime(value))
];
const counts = {
  total: checks.length,
  pass: checks.filter(check => check.status === 'pass').length,
  fail: checks.filter(check => check.status === 'fail').length,
  pending: checks.filter(check => check.status === 'pending').length
};
process.stdout.write(JSON.stringify({ request, kind, fixture, counts, checks }));
"#;

const OPTION_MATRIX_PROBE_SCRIPT: &str = r#"
const path = require('path');
const { createRequire } = require('module');

const root = process.env.VUEC_OPTION_ROOT;
const side = process.env.VUEC_OPTION_SIDE;
const payload = JSON.parse(process.env.VUEC_OPTION_PAYLOAD || '{}');
const rootRequire = createRequire(path.join(root, 'package.json'));
const request = payload.request;

function load() {
  return rootRequire(request);
}

function capture(fn) {
  try {
    return { ok: true, value: normalize(fn()) };
  } catch (error) {
    return {
      ok: false,
      error: {
        name: error && error.name ? String(error.name) : null,
        code: error && error.code ? String(error.code) : null,
        message: normalizeMessage(error && error.message ? error.message : String(error))
      }
    };
  }
}

function normalizeMessage(message) {
  return String(message)
    .replaceAll(root.replace(/\\/g, '/'), '<option-root>')
    .replace(/\\/g, '/');
}

function normalize(value, seen = new WeakSet()) {
  if (value === undefined) return { __type: 'undefined' };
  if (value === null) return null;
  if (typeof value === 'symbol') return { __type: 'symbol', description: value.description || null };
  if (typeof value === 'function') return { __type: 'function', name: value.name, length: value.length };
  if (value instanceof Set) {
    return Array.from(value)
      .map(item => normalize(item, seen))
      .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
  }
  if (typeof value !== 'object') return value;
  if (seen.has(value)) return { __type: 'cycle' };
  seen.add(value);
  if (Array.isArray(value)) return value.map(item => normalize(item, seen));
  const out = {};
  for (const key of Object.keys(value).sort()) {
    out[key] = normalize(value[key], seen);
  }
  return out;
}

function pathValue(value, optionPath) {
  if (!optionPath) return value;
  const segments = optionPath.split('.');
  let cursor = value;
  for (const segment of segments) {
    if (cursor == null || typeof cursor !== 'object') return undefined;
    cursor = cursor[segment];
  }
  return cursor;
}

function cloneOptionValue(optionValue) {
  if (optionValue === null || optionValue === undefined) return optionValue;
  return JSON.parse(JSON.stringify(optionValue));
}

function normalizeOptionValue(optionValue) {
  const value = cloneOptionValue(optionValue);
  if (
    side === 'official' &&
    payload.target_package === '@vue/compiler-dom' &&
    payload.option_name === 'isCustomElement' &&
    value &&
    Array.isArray(value.isCustomElement)
  ) {
    const customElements = new Set(value.isCustomElement);
    value.isCustomElement = tag => customElements.has(tag);
  }
  if (
    side === 'official' &&
    payload.target_package === 'vue-template-compiler' &&
    payload.option_name === 'directives' &&
    value &&
    value.directives &&
    typeof value.directives === 'object'
  ) {
    for (const key of Object.keys(value.directives)) {
      if (value.directives[key] === true) {
        value.directives[key] = () => true;
      }
    }
  }
  return value;
}

function optionsArg() {
  switch (payload.input_kind || 'value') {
    case 'missing':
      return { present: false, value: undefined };
    case 'undefined':
      return { present: true, value: undefined };
    case 'null':
      return { present: true, value: null };
    default:
      return { present: true, value: normalizeOptionValue(payload.option_value) };
  }
}

function optionObjectWithSource(baseSource) {
  const arg = optionsArg();
  const objectValue = arg.value && typeof arg.value === 'object' ? arg.value : {};
  return Object.assign({ source: baseSource }, objectValue);
}

function extractStyleSource(fixture) {
  const match = String(fixture).match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  return match ? match[1] : fixture;
}

function extractTemplateSource(fixture) {
  const match = String(fixture).match(/<template[^>]*>([\s\S]*?)<\/template>/i);
  return match ? match[1] : fixture;
}

function isVue27Sfc() {
  return payload.target_version_line === 'vue2_7' && payload.target_entry === 'vue/compiler-sfc';
}

function normalizeSfcStyleResult(result) {
  if (!result || typeof result !== 'object') return result;
  const out = Object.assign({}, result);
  if (out.rawResult && !Array.isArray(out.rawResult)) {
    out.rawResult = ['postcss-result'];
  }
  if (out.map === undefined) {
    out.map = null;
  }
  if (out.dependencies instanceof Set) {
    out.dependencies = Array.from(out.dependencies).sort();
  }
  return out;
}

function invoke(api) {
  const method = payload.method;
  const fixture = payload.source;
  const arg = optionsArg();
  switch (method) {
    case 'compile':
      return capture(() => arg.present ? api.compile(fixture, arg.value) : api.compile(fixture));
    case 'compileToFunctions':
      return capture(() => arg.present ? api.compileToFunctions(fixture, arg.value, {}) : api.compileToFunctions(fixture));
    case 'parse':
      if (isVue27Sfc()) {
        const value = Object.assign({ source: fixture }, arg.value && typeof arg.value === 'object' ? arg.value : {});
        return capture(() => arg.present ? api.parse(value) : api.parse({ source: fixture }));
      }
      return capture(() => arg.present ? api.parse(fixture, arg.value) : api.parse(fixture));
    case 'compileTemplate':
      if (isVue27Sfc()) {
        return capture(() => api.compileTemplate(Object.assign(optionObjectWithSource(extractTemplateSource(fixture)), arg.value && typeof arg.value === 'object' ? arg.value : {})));
      }
      return capture(() => api.compileTemplate(optionObjectWithSource(fixture)));
    case 'compileScript': {
      return capture(() => {
        const parsed = isVue27Sfc()
          ? api.parse({ source: fixture, filename: 'contract.vue' })
          : api.parse(fixture, { filename: 'contract.vue' });
        const descriptor = parsed && parsed.descriptor ? parsed.descriptor : parsed;
        return arg.present ? api.compileScript(descriptor, arg.value) : api.compileScript(descriptor);
      });
    }
    case 'compileStyle':
      return capture(() => normalizeSfcStyleResult(api.compileStyle(optionObjectWithSource(extractStyleSource(fixture)))));
    case 'baseCompile':
      return capture(() => arg.present ? api.baseCompile(fixture, arg.value) : api.baseCompile(fixture));
    case 'baseParse':
      return capture(() => arg.present ? api.baseParse(fixture, arg.value) : api.baseParse(fixture));
    default:
      throw new Error(`unknown option matrix method ${method}`);
  }
}

const api = load();
const result = invoke(api);
const normalized = {
  side,
  request,
  method: payload.method,
  fixture_id: payload.fixture_id,
  option_name: payload.option_name,
  option_path: payload.option_path,
  ok: result.ok,
  value: result.ok ? result.value : null,
  error: result.ok ? null : result.error,
};
process.stdout.write(JSON.stringify(normalized));
"#;
