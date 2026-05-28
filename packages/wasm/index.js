let wasm;

function normalizeOptions(options) {
  return JSON.stringify(options || {});
}

function parseJson(value) {
  return JSON.parse(value);
}

export async function init(input) {
  if (!wasm) {
    const mod = await import(input || defaultWasmModulePath());
    wasm = mod && mod.default && typeof mod.default === 'object' ? mod.default : mod;
  }
  if (typeof wasm.default === 'function') {
    await wasm.default();
  }
  return api;
}

function defaultWasmModulePath() {
  return isNodeRuntime() ? './pkg-node/vuec_wasm.js' : './pkg/vuec_wasm.js';
}

function isNodeRuntime() {
  return typeof process !== 'undefined'
    && process.versions
    && typeof process.versions.node === 'string';
}

function ensureWasm() {
  if (!wasm) {
    throw new Error('@vuec-rs/wasm is not initialized; call init() first');
  }
  return wasm;
}

export function version() {
  return ensureWasm().version();
}

export function compileVue2(template, options = {}) {
  return parseJson(ensureWasm().compileVue2(String(template || ''), normalizeOptions(options)));
}

export function compileVue3Dom(source, options = {}) {
  return parseJson(ensureWasm().compileVue3Dom(String(source || ''), normalizeOptions(options)));
}

export function compileVue3Ssr(source, options = {}) {
  return parseJson(ensureWasm().compileVue3Ssr(String(source || ''), normalizeOptions(options)));
}

export function parseSfc(source, options = {}) {
  return parseJson(ensureWasm().parseSfc(String(source || ''), normalizeOptions(options)));
}

export function compileSfcTemplate(source, options = {}) {
  return parseJson(ensureWasm().compileSfcTemplate(String(source || ''), normalizeOptions(options)));
}

export function compileSfcTemplateSource(source, options = {}) {
  return parseJson(ensureWasm().compileSfcTemplateSource(String(source || ''), normalizeOptions(options)));
}

export function compileSfcScript(source, options = {}) {
  return parseJson(ensureWasm().compileSfcScript(String(source || ''), normalizeOptions(options)));
}

export function compileSfcStyle(source, options = {}) {
  return parseJson(ensureWasm().compileSfcStyle(String(source || ''), normalizeOptions(options)));
}

export function compile(template, options = {}) {
  return compileVue2(template, options);
}

export function compileDom(source, options = {}) {
  return compileVue3Dom(source, options);
}

export function compileSsr(source, options = {}) {
  return compileVue3Ssr(source, options);
}

export function parse(source, options = {}) {
  return parseSfc(source, options);
}

export function compileTemplate(options = {}) {
  return compileSfcTemplateSource(String(options.source || ''), options);
}

export function compileScript(descriptor, options = {}) {
  const source = descriptor && typeof descriptor.source === 'string' ? descriptor.source : '';
  return compileSfcScript(source, {
    filename: descriptor && descriptor.filename,
    ...options,
  });
}

export function compileStyle(options = {}) {
  return compileSfcStyle(styleSfcSource(String(options.source || ''), options), options);
}

function styleSfcSource(source, options) {
  if (/<style(?:\s|>)/i.test(source)) {
    return source;
  }
  const attrs = [];
  if (options.scoped) {
    attrs.push('scoped');
  }
  if (options.modules || options.module) {
    attrs.push(
      options.module && typeof options.module === 'string'
        ? `module="${escapeAttribute(options.module)}"`
        : 'module'
    );
  }
  const lang = options.lang || options.preprocessLang || options.preprocess_lang;
  if (lang) {
    attrs.push(`lang="${escapeAttribute(lang)}"`);
  }
  return `<style${attrs.length ? ` ${attrs.join(' ')}` : ''}>${source}</style>`;
}

function escapeAttribute(value) {
  return String(value).replace(/&/g, '&amp;').replace(/"/g, '&quot;');
}

export const api = {
  init,
  version,
  compileVue2,
  compileVue3Dom,
  compileVue3Ssr,
  parseSfc,
  compileSfcTemplate,
  compileSfcTemplateSource,
  compileSfcScript,
  compileSfcStyle,
  compile,
  compileDom,
  compileSsr,
  parse,
  compileTemplate,
  compileScript,
  compileStyle,
};
