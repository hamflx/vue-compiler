'use strict';

const native = require('@vuec-rs/native');

function parse(input) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  const opts = options || {};
  if (input && typeof input === 'object') {
    return normalizeVue27ParseComponentResult(native.parseVue27SfcComponent(String(input.source || ''), {
      ...opts,
      filename: input.filename || opts.filename,
    }));
  }
  return normalizeVue27ParseComponentResult(native.parseVue27SfcComponent(String(input || ''), opts));
}

function parseComponent(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  return normalizeVue27ParseComponentResult(native.parseVue27SfcComponent(String(source || ''), options || {}));
}

function compileTemplate(options) {
  const opts = options || {};
  const source = String(opts.source || '');
  return native.compileVue27SfcTemplate(source, opts);
}

function compileScript(descriptor) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  const source = descriptor && typeof descriptor.source === 'string' ? descriptor.source : '';
  return normalizeVue27ScriptResult(native.compileVue27SfcScript(source, vue27ScriptOptions({
    filename: descriptor && descriptor.filename,
    ...(options || {}),
  })), descriptor || {});
}

function compileStyle(options) {
  const raw = options || {};
  const opts = vue27StyleOptions(resolveStylePreprocessOptions(String(raw.source || ''), raw));
  const result = normalizeVue27StyleResult(native.compileStyle(vue27StyleNativeOptions(opts)));
  return applyVue27StylePostcssSync(result, opts);
}

function compileStyleAsync(options) {
  const raw = options || {};
  const opts = vue27StyleOptions(resolveStylePreprocessOptions(String(raw.source || ''), raw));
  const result = normalizeVue27StyleResult(native.compileStyle(vue27StyleNativeOptions(opts)));
  return applyVue27StylePostcssAsync(result, opts);
}

function rewriteDefault(source, variable, parserPlugins) {
  return native.rewriteDefaultVue27(String(source || ''), String(variable || ''), parserPlugins || []);
}

function generateCodeFrame(source) {
  const start = arguments.length > 1 ? arguments[1] : undefined;
  const end = arguments.length > 2 ? arguments[2] : undefined;
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

function normalizeVue27StyleResult(result) {
  if (!result || typeof result !== 'object') return result;
  const out = { ...result };
  delete out.dependencies;
  if (out.map === null) out.map = undefined;
  return out;
}

function vue27ScriptOptions(options) {
  const out = { ...(options || {}) };
  if (typeof __TEST__ !== 'undefined' && __TEST__ === true) {
    out.__vuecEmitScriptSetupMarker = false;
  }
  return out;
}

function vue27StyleOptions(options) {
  const out = { ...options };
  if (!Object.prototype.hasOwnProperty.call(out, 'scoped')) out.scoped = true;
  return out;
}

function resolveStylePreprocessOptions(source, options) {
  if (!options || !options.preprocessOptions || typeof options.preprocessOptions !== 'object') {
    return options;
  }
  const preprocessOptions = options.preprocessOptions;
  if (typeof preprocessOptions.additionalData !== 'function') {
    return options;
  }
  return {
    ...options,
    preprocessOptions: {
      ...preprocessOptions,
      additionalData: preprocessOptions.additionalData(source, options.filename),
    },
  };
}

function vue27StyleNativeOptions(options) {
  const out = {
    __vuecCssVarNameStyle: 'vue27Legacy',
    __vuecCssVarIgnoreLineComments: false,
    __vuecWarnDeprecatedScopedSelectors: false,
  };
  for (const key of Object.keys(options || {})) {
    if (
      key !== 'postcssPlugins' &&
      key !== 'postcssOptions' &&
      key !== 'sourceMap' &&
      key !== 'source_map'
    ) {
      out[key] = options[key];
    }
  }
  return out;
}

function vue27StylePostcssRequired(options) {
  return !!(
    options &&
    (Array.isArray(options.postcssPlugins) || options.postcssOptions)
  );
}

function vue27StylePostcssOptions(options) {
  const postcssOptions = Object.assign({}, options && options.postcssOptions ? options.postcssOptions : {});
  const filename = options && options.filename ? options.filename : undefined;
  if (filename !== undefined) {
    if (postcssOptions.to === undefined) postcssOptions.to = filename;
    if (postcssOptions.from === undefined) postcssOptions.from = filename;
  }
  return postcssOptions;
}

function applyVue27StylePostcssSync(result, options) {
  if (!vue27StylePostcssRequired(options)) return result;
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors.slice() : [];
  let rawResult;
  try {
    const postcss = require('postcss');
    rawResult = postcss((options && options.postcssPlugins) || []).process(
      out.code || '',
      vue27StylePostcssOptions(options)
    );
    out.code = rawResult.css || '';
    out.map = rawResult.map && rawResult.map.toJSON ? rawResult.map.toJSON() : out.map;
  } catch (error) {
    errors.push(error);
  }
  out.errors = errors;
  out.rawResult = rawResult;
  return out;
}

function applyVue27StylePostcssAsync(result, options) {
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors.slice() : [];
  if (!vue27StylePostcssRequired(options)) {
    return Promise.resolve(out);
  }
  try {
    const postcss = require('postcss');
    const rawResult = postcss((options && options.postcssPlugins) || []).process(
      out.code || '',
      vue27StylePostcssOptions(options)
    );
    return Promise.resolve(rawResult)
      .then(postcssResult => {
        out.code = postcssResult.css || '';
        out.map = postcssResult.map && postcssResult.map.toJSON ? postcssResult.map.toJSON() : out.map;
        out.errors = errors;
        out.rawResult = postcssResult;
        return out;
      })
      .catch(error => ({
        code: '',
        map: undefined,
        errors: errors.concat(error && error.message ? error.message : error),
        rawResult: undefined,
      }));
  } catch (error) {
    return Promise.resolve({
      code: '',
      map: undefined,
      errors: errors.concat(error && error.message ? error.message : error),
      rawResult: undefined,
    });
  }
}

function normalizeVue27Descriptor(descriptor) {
  if (!descriptor || typeof descriptor !== 'object') return descriptor;
  return {
    source: descriptor.source || '',
    filename: descriptor.filename || 'anonymous.vue',
    template: descriptor.template ? normalizeVue27Block(descriptor, descriptor.template, false) : null,
    script: descriptor.script ? normalizeVue27Block(descriptor, descriptor.script, false) : null,
    scriptSetup: descriptor.script_setup ? normalizeVue27Block(descriptor, descriptor.script_setup, false) : null,
    styles: Array.isArray(descriptor.styles)
      ? descriptor.styles.map(block => normalizeVue27Block(descriptor, block, true))
      : [],
    customBlocks: Array.isArray(descriptor.custom_blocks)
      ? descriptor.custom_blocks.map(block => normalizeVue27Block(descriptor, block, false))
      : [],
    cssVars: vue27CssVars(descriptor),
    errors: [],
    shouldForceReload() {
      return false;
    },
  };
}

function normalizeVue27ParseComponentResult(result) {
  if (!result || typeof result !== 'object') return result;
  const descriptor = normalizeVue27Descriptor(result.descriptor || result);
  descriptor.errors = Array.isArray(result.errors) ? result.errors : [];
  return descriptor;
}

function normalizeVue27Block(descriptor, block, style) {
  const out = {
    type: block.type_name || block.type || '',
    content: block.content || '',
    start: blockContentStart(block, descriptor),
    end: blockContentEnd(block, descriptor),
    attrs: vue27Attrs(block.attrs),
  };
  if (out.type === 'script' || out.type === 'style') {
    out.map = vue27BlockMap(descriptor);
  }
  if (block.attrs && block.attrs.setup) out.setup = true;
  if (block.attrs && block.attrs.lang) out.lang = block.attrs.lang;
  if (block.attrs && block.attrs.src) out.src = block.attrs.src;
  if (block.attrs && block.attrs.module != null) out.module = block.attrs.module === '' ? true : block.attrs.module;
  if (style && block.attrs && block.attrs.scoped) out.scoped = true;
  return out;
}

function vue27Attrs(attrs) {
  const raw = attrs && attrs.raw && typeof attrs.raw === 'object' ? attrs.raw : {};
  const out = {};
  for (const key of Object.keys(raw)) {
    out[key] = raw[key];
  }
  if (attrs && attrs.scoped) out.scoped = true;
  if (attrs && attrs.setup) out.setup = true;
  if (attrs && attrs.lang) out.lang = attrs.lang;
  if (attrs && attrs.src) out.src = attrs.src;
  if (attrs && attrs.module != null) out.module = attrs.module === '' ? true : attrs.module;
  return out;
}

function blockContentStart(block, descriptor) {
  if (typeof block.content_start === 'number') return block.content_start;
  const source = descriptor && typeof descriptor.source === 'string' ? descriptor.source : '';
  if (block && block.loc && typeof block.loc.start === 'number') {
    const openEnd = source.indexOf('>', block.loc.start);
    if (openEnd >= 0 && openEnd < block.loc.end) return openEnd + 1;
    return block.loc.start;
  }
  return 0;
}

function blockContentEnd(block, descriptor) {
  if (typeof block.content_end === 'number') return block.content_end;
  const start = blockContentStart(block, descriptor);
  if (block && typeof block.content === 'string') return start + block.content.length;
  return block && block.loc && typeof block.loc.end === 'number' ? block.loc.end : 0;
}

function vue27BlockMap(descriptor) {
  const filename = descriptor && descriptor.filename ? descriptor.filename : 'anonymous.vue';
  const source = descriptor && descriptor.source ? descriptor.source : '';
  return {
    version: 3,
    sources: [filename],
    names: [],
    mappings: 'AAAA',
    file: filename,
    sourceRoot: '',
    sourcesContent: [source],
  };
}

function vue27CssVars(descriptor) {
  const vars = [];
  const styles = Array.isArray(descriptor && descriptor.styles) ? descriptor.styles : [];
  for (const style of styles) {
    const content = String(style && style.content || '');
    const pattern = /v-bind\s*\(\s*([^)]+?)\s*\)/g;
    let match;
    while ((match = pattern.exec(content))) {
      const value = match[1].trim().replace(/^['"]|['"]$/g, '');
      if (value && !vars.includes(value)) vars.push(value);
    }
  }
  return vars;
}

function normalizeVue27ScriptResult(result, descriptor) {
  if (!result || typeof result !== 'object') return result;
  if (Array.isArray(result.errors) && result.errors.length > 0) {
    const message = result.errors.map(error => String(error)).join('\n');
    throw new Error(message);
  }
  const out = { ...result };
  delete out.errors;
  delete out.deps;
  const sourceBlock = descriptor && (descriptor.scriptSetup || descriptor.script_setup || descriptor.script);
  if (sourceBlock) {
    out.start = sourceBlock.start;
    out.end = sourceBlock.end;
  } else if (out.loc) {
    out.start = blockContentStart({ loc: out.loc }, descriptor);
    out.end = blockContentEnd({ loc: out.loc, content: '' }, descriptor);
  }
  delete out.loc;
  out.attrs = vue27Attrs(out.attrs);
  if (out.bindings && typeof out.bindings === 'object') {
    const isScriptSetup = out.bindings.__isScriptSetup === true || out.bindings.__isScriptSetup === 'true';
    delete out.bindings.__isScriptSetup;
    Object.defineProperty(out.bindings, '__isScriptSetup', {
      enumerable: false,
      configurable: true,
      value: isScriptSetup,
    });
  }
  out.imports = {};
  return out;
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
