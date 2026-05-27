'use strict';

const native = require('@vuec-rs/native');

let packageVersion = '0.0.0-vuec-napi';
try {
  packageVersion = require('../package.json').version || packageVersion;
} catch (_) {}

let registeredTS = null;

class MagicString {
  constructor(original) {
    this.original = String(original || '');
    this.intro = '';
    this.outro = '';
    this.edits = [];
  }

  append(content) {
    this.outro += String(content);
    return this;
  }

  prepend(content) {
    this.intro = String(content) + this.intro;
    return this;
  }

  overwrite(start, end, content) {
    this.edits.push({ start: Number(start) || 0, end: Number(end) || 0, content: String(content) });
    return this;
  }

  remove(start, end) {
    return this.overwrite(start, end, '');
  }

  slice(start, end) {
    return this.original.slice(start, end);
  }

  toString() {
    const sorted = this.edits.slice().sort((a, b) => a.start - b.start || a.end - b.end);
    let cursor = 0;
    let output = this.intro;
    for (const edit of sorted) {
      const start = Math.max(0, Math.min(this.original.length, edit.start));
      const end = Math.max(start, Math.min(this.original.length, edit.end));
      if (start < cursor) {
        continue;
      }
      output += this.original.slice(cursor, start);
      output += edit.content;
      cursor = end;
    }
    output += this.original.slice(cursor);
    output += this.outro;
    return output;
  }

  generateMap() {
    return { version: 3, sources: [], names: [], mappings: '', sourcesContent: [] };
  }
}

MagicString.Bundle = undefined;
MagicString.SourceMap = undefined;
MagicString.default = MagicString;

function parse(source, options) {
  const parser = loadBabelParser();
  if (parser && typeof parser.parse === 'function') {
    return parser.parse(source, options);
  }
  return minimalBabelParse(source, options);
}

function parse$1(source) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  return { descriptor: native.parseSfc(String(source || ''), options || {}), errors: [] };
}

function compileTemplate(options) {
  return native.compileTemplate(options || {});
}

function compileScript(descriptor, options) {
  return native.compileScript(descriptor || {}, options || {});
}

function compileStyle(options) {
  return native.compileStyle(options || {});
}

function compileStyleAsync(options) {
  return Promise.resolve(compileStyle(options || {}));
}

function generateCodeFrame(source) {
  const start = arguments.length > 1 ? arguments[1] : undefined;
  const end = arguments.length > 2 ? arguments[2] : undefined;
  return native.generateCodeFrameVue2(String(source || ''), start || 0, end || start || 0);
}

function rewriteDefault(source, as, parserPlugins) {
  return native.rewriteDefaultVue27(String(source || ''), String(as || ''), parserPlugins || []);
}

function rewriteDefaultAST(ast, s, as) {
  const body = Array.isArray(ast) ? ast : ast && ast.program && Array.isArray(ast.program.body) ? ast.program.body : [];
  let found = false;
  for (const node of body) {
    if (node && node.type === 'ExportDefaultDeclaration') {
      found = true;
      const declaration = node.declaration || {};
      if (declaration.type === 'ClassDeclaration' && declaration.id && declaration.id.name) {
        const start = Array.isArray(declaration.decorators) && declaration.decorators.length
          ? declaration.decorators[declaration.decorators.length - 1].end
          : node.start;
        s.overwrite(start || node.start || 0, declaration.id.start || node.start || 0, ' class ');
        s.append(`\nconst ${as} = ${declaration.id.name}`);
      } else {
        s.overwrite(node.start || 0, declaration.start || node.start || 0, `const ${as} = `);
      }
    } else if (node && node.type === 'ExportNamedDeclaration' && Array.isArray(node.specifiers)) {
      for (const specifier of node.specifiers) {
        const exported = specifier && specifier.exported;
        if (exported && exported.type === 'Identifier' && exported.name === 'default') {
          found = true;
          const local = specifier.local && specifier.local.name ? specifier.local.name : 'default';
          if (node.source && node.source.value) {
            s.prepend(`import { ${local} as __VUE_DEFAULT__ } from '${node.source.value}'\n`);
            s.append(`\nconst ${as} = __VUE_DEFAULT__`);
          } else {
            s.append(`\nconst ${as} = ${local}`);
          }
          s.remove(specifier.start || node.start || 0, specifier.end || node.end || 0);
        }
      }
    }
  }
  if (!found) {
    s.append(`\nconst ${as} = {}`);
  }
}

function extractIdentifiers(param) {
  const out = [];
  collectIdentifiers(param, out, { includeMembers: false });
  return out;
}

function walkIdentifiers(root, onIdentifier) {
  const includeAll = arguments.length > 2 ? arguments[2] : false;
  const projection = native.callVue3CoreProjection('vue3.core.walkIdentifiers', {
    root,
    includeAll,
  });
  for (const event of projection.identifiers || []) {
    const node = nodeAtPath(root, event.path);
    if (!node) continue;
    const parent = event.parentPath ? nodeAtPath(root, event.parentPath) : null;
    const stack = (event.parentStackPaths || [])
      .map(path => nodeAtPath(root, path))
      .filter(Boolean);
    onIdentifier(node, parent, stack, !!event.isReferenced, !!event.isLocal);
  }
}

function walk(root, enter) {
  const leave = arguments.length > 2 ? arguments[2] : undefined;
  return walkAst(root, (node, parent) => {
    if (typeof enter === 'function') enter(node, parent);
  }, (node, parent) => {
    if (typeof leave === 'function') leave(node, parent);
  });
}

function extractRuntimeProps(ctx) {
  if (!ctx || !ctx.propsTypeDecl) {
    return ctx && ctx.propsRuntimeDecl ? stringifyNode(ctx.propsRuntimeDecl, ctx) : undefined;
  }
  const elements = resolveTypeElements(ctx, ctx.propsTypeDecl);
  const props = Object.keys(elements.props || {});
  if (!props.length) return undefined;
  return `{ ${props.map(name => `${JSON.stringify(name)}: { type: null }`).join(', ')} }`;
}

function extractRuntimeEmits(ctx) {
  if (!ctx || !ctx.emitsTypeDecl) {
    return ctx && ctx.emitsRuntimeDecl ? stringifyNode(ctx.emitsRuntimeDecl, ctx) : undefined;
  }
  const elements = resolveTypeElements(ctx, ctx.emitsTypeDecl);
  const names = Object.keys(elements.props || {});
  return names.length ? `[${names.map(JSON.stringify).join(', ')}]` : undefined;
}

function inferRuntimeType(ctx, node) {
  switch (node && node.type) {
    case 'TSStringKeyword':
    case 'StringLiteral':
      return ['String'];
    case 'TSNumberKeyword':
    case 'NumericLiteral':
    case 'BigIntLiteral':
      return ['Number'];
    case 'TSBooleanKeyword':
    case 'BooleanLiteral':
      return ['Boolean'];
    case 'TSArrayType':
    case 'TSTupleType':
      return ['Array'];
    case 'TSFunctionType':
    case 'TSCallSignatureDeclaration':
    case 'TSMethodSignature':
      return ['Function'];
    case 'TSTypeLiteral':
    case 'TSInterfaceDeclaration':
    case 'TSObjectKeyword':
    case 'ClassDeclaration':
      return ['Object'];
    case 'TSNullKeyword':
      return ['null'];
    case 'TSLiteralType':
      return inferRuntimeType(ctx, node.literal);
    case 'TSUnionType': {
      const set = new Set();
      for (const ty of node.types || []) {
        for (const item of inferRuntimeType(ctx, ty)) set.add(item);
      }
      return Array.from(set);
    }
    case 'TSIntersectionType':
      return Array.from(new Set([].concat(...(node.types || []).map(ty => inferRuntimeType(ctx, ty))))).filter(type => type !== 'Unknown');
    case 'TSTypeReference': {
      const name = getReferenceName(node);
      if (['Array', 'Function', 'Object', 'Set', 'Map', 'WeakSet', 'WeakMap', 'Date', 'Promise', 'Error'].includes(name)) {
        return [name];
      }
      if (['Partial', 'Required', 'Readonly', 'Record', 'Pick', 'Omit', 'InstanceType'].includes(name)) {
        return ['Object'];
      }
      if (['Parameters', 'ConstructorParameters', 'ReadonlyArray'].includes(name)) {
        return ['Array'];
      }
      if (['Uppercase', 'Lowercase', 'Capitalize', 'Uncapitalize'].includes(name)) {
        return ['String'];
      }
      return ['Unknown'];
    }
    default:
      return ['Unknown'];
  }
}

function resolveTypeElements(ctx, node, scope, typeParameters) {
  const props = Object.create(null);
  const calls = [];
  const members = node && node.type === 'TSTypeLiteral'
    ? node.members || []
    : node && node.type === 'TSInterfaceDeclaration' && node.body
      ? node.body.body || []
      : [];
  for (const member of members) {
    if (!member) continue;
    if (member.type === 'TSCallSignatureDeclaration' || member.type === 'TSFunctionType') {
      calls.push(member);
      continue;
    }
    if (member.type === 'TSPropertySignature' || member.type === 'TSMethodSignature') {
      const name = staticKeyName(member.key);
      if (name != null) props[name] = member;
    }
  }
  const result = { props };
  if (calls.length) result.calls = calls;
  return result;
}

function invalidateTypeCache(ctx) {
  for (const key of Object.keys(parseCache)) {
    parseCache[key] = undefined;
  }
}

function registerTS(ts) {
  registeredTS = ts || null;
}

function namedNoPrototype(name, arity, fn) {
  const bound = fn.bind(null);
  Object.defineProperty(bound, 'name', { value: name, configurable: true });
  Object.defineProperty(bound, 'length', { value: arity, configurable: true });
  return bound;
}

const isStaticProperty = namedNoPrototype('isStaticProperty', 1, function (node) {
  return !!(node && (node.type === 'ObjectProperty' || node.type === 'Property' || node.type === 'ObjectMethod' || node.type === 'ClassProperty' || node.type === 'PropertyDefinition') && !node.computed);
});

function isInDestructureAssignment(parent, parentStack) {
  if (!parent) return false;
  if (parent.type === 'ObjectPattern' || parent.type === 'ArrayPattern') return true;
  return Array.isArray(parentStack) && parentStack.some(node => node && (node.type === 'ObjectPattern' || node.type === 'ArrayPattern'));
}

const shouldTransformRef = namedNoPrototype('shouldTransformRef', 0, function () {
  return false;
});

const parseCache = {};
[
  'allowStale',
  'allowStaleOnFetchAbort',
  'allowStaleOnFetchRejection',
  'ignoreFetchAbort',
  'maxEntrySize',
  'noDeleteOnFetchRejection',
  'noDeleteOnStaleGet',
  'noDisposeOnSet',
  'noUpdateTTL',
  'sizeCalculation',
  'ttl',
  'ttlAutopurge',
  'ttlResolution',
  'updateAgeOnGet',
  'updateAgeOnHas',
].forEach(key => {
  parseCache[key] = undefined;
});

const errorMessages = {};
for (let i = 0; i <= 64; i++) {
  errorMessages[i] = `compiler error ${i}`;
}

function loadBabelParser() {
  try {
    return require('@babel/parser');
  } catch (_) {
    return null;
  }
}

function minimalBabelParse(source, options) {
  const text = String(source || '');
  return {
    type: 'File',
    program: {
      type: 'Program',
      sourceType: options && options.sourceType ? options.sourceType : 'module',
      body: parseTopLevelStatements(text),
    },
  };
}

function parseTopLevelStatements(source) {
  const statements = [];
  const exportDefault = /\bexport\s+default\s+/g;
  let match;
  while ((match = exportDefault.exec(source))) {
    const declarationStart = match.index + match[0].length;
    const declaration = parseDefaultDeclaration(source, declarationStart);
    statements.push({
      type: 'ExportDefaultDeclaration',
      start: match.index,
      end: declaration.end,
      declaration,
    });
  }

  const exportNamed = /\bexport\s*\{([\s\S]*?)\}(?:\s*from\s*(['"])(.*?)\2)?/g;
  while ((match = exportNamed.exec(source))) {
    const specifiers = [];
    let cursor = match.index + match[0].indexOf('{') + 1;
    for (const rawPart of match[1].split(',')) {
      const part = rawPart.trim();
      if (!part) {
        cursor += rawPart.length + 1;
        continue;
      }
      const asMatch = part.match(/^(.+?)\s+as\s+(.+)$/);
      const local = (asMatch ? asMatch[1] : part).trim();
      const exported = (asMatch ? asMatch[2] : part).trim();
      const partStart = source.indexOf(part, cursor);
      const localStart = partStart + part.indexOf(local);
      const exportedStart = partStart + part.lastIndexOf(exported);
      specifiers.push({
        type: 'ExportSpecifier',
        start: partStart,
        end: partStart + part.length,
        local: identifierNode(local, localStart),
        exported: identifierNode(exported, exportedStart),
      });
      cursor = partStart + part.length + 1;
    }
    statements.push({
      type: 'ExportNamedDeclaration',
      start: match.index,
      end: match.index + match[0].length,
      specifiers,
      source: match[3] ? { type: 'StringLiteral', value: match[3] } : null,
    });
  }
  statements.sort((a, b) => a.start - b.start);
  return statements;
}

function parseDefaultDeclaration(source, start) {
  const tail = source.slice(start);
  const classMatch = tail.match(/^\s*class\s+([A-Za-z_$][\w$]*)/);
  if (classMatch) {
    const classStart = start + classMatch[0].indexOf('class');
    const nameStart = start + classMatch[0].lastIndexOf(classMatch[1]);
    return {
      type: 'ClassDeclaration',
      start: classStart,
      end: findStatementEnd(source, start),
      id: identifierNode(classMatch[1], nameStart),
      decorators: [],
    };
  }
  return {
    type: 'Expression',
    start,
    end: findStatementEnd(source, start),
  };
}

function findStatementEnd(source, start) {
  const semicolon = source.indexOf(';', start);
  const newline = source.indexOf('\n', start);
  const end = [semicolon, newline].filter(index => index >= 0).sort((a, b) => a - b)[0];
  return end == null ? source.length : end + (end === semicolon ? 1 : 0);
}

function identifierNode(name, start) {
  return {
    type: 'Identifier',
    name,
    start,
    end: start + String(name).length,
  };
}

function collectIdentifiers(node, out, options) {
  walkAst(node, current => {
    if (current && current.type === 'Identifier') {
      out.push(current);
    } else if (options && options.includeMembers && current && current.type === 'MemberExpression') {
      collectIdentifiers(current.object, out, options);
    }
  });
}

function walkAst(root, enter, leave, parent, stack) {
  if (!root || typeof root !== 'object') return root;
  const parents = stack || [];
  if (Array.isArray(root)) {
    for (const item of root) walkAst(item, enter, leave, parent, parents);
    return root;
  }
  if (enter) enter(root, parent || null, parents);
  const nextStack = parent ? parents.concat(parent) : parents;
  for (const key of Object.keys(root)) {
    if (key === 'parent' || key === 'loc') continue;
    const value = root[key];
    if (value && typeof value === 'object') {
      walkAst(value, enter, leave, root, nextStack);
    }
  }
  if (leave) leave(root, parent || null, parents);
  return root;
}

function nodeAtPath(root, path) {
  let node = root;
  for (const segment of path || []) {
    if (node == null) return null;
    node = node[segment];
  }
  return node || null;
}

function isBindingIdentifier(node, parent) {
  if (!node || !parent) return false;
  if (parent.id === node && /Declaration$/.test(parent.type || '')) return true;
  if ((parent.type === 'ObjectProperty' || parent.type === 'Property') && parent.key === node && !parent.computed) return false;
  return parent.type === 'ObjectPattern' || parent.type === 'ArrayPattern' || parent.type === 'RestElement' || parent.type === 'AssignmentPattern';
}

function getReferenceName(node) {
  const typeName = node && node.typeName;
  if (!typeName) return '';
  if (typeName.type === 'Identifier') return typeName.name;
  if (typeName.type === 'TSQualifiedName') return `${getReferenceName({ typeName: typeName.left })}.${typeName.right && typeName.right.name}`;
  return '';
}

function staticKeyName(node) {
  if (!node) return null;
  if (node.type === 'Identifier') return node.name;
  if (node.type === 'StringLiteral' || node.type === 'NumericLiteral') return String(node.value);
  return null;
}

function stringifyNode(node, ctx) {
  if (!node) return undefined;
  if (typeof node === 'string') return node;
  if (typeof ctx.getString === 'function' && node.start != null && node.end != null) {
    return ctx.getString(node);
  }
  if (node.content) return String(node.content);
  return undefined;
}

module.exports = {
  MagicString,
  babelParse: parse,
  compileScript,
  compileStyle,
  compileStyleAsync,
  compileTemplate,
  errorMessages,
  extractIdentifiers,
  extractRuntimeEmits,
  extractRuntimeProps,
  generateCodeFrame,
  inferRuntimeType,
  invalidateTypeCache,
  isInDestructureAssignment,
  isStaticProperty,
  parse: parse$1,
  parseCache,
  registerTS,
  resolveTypeElements,
  rewriteDefault,
  rewriteDefaultAST,
  shouldTransformRef,
  version: packageVersion,
  walk,
  walkIdentifiers,
};
