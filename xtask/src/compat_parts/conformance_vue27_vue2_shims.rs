fn write_vue27_sfc_source_shims(prepared_root: &Path) -> Result<()> {
    let sfc_src = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("src");
    fs::create_dir_all(&sfc_src)
        .with_context(|| format!("failed to create {}", sfc_src.display()))?;
    write_text(
        &sfc_src.join("index.ts"),
        "export * from 'vue/compiler-sfc'\n",
    )?;
    for module in [
        "parse",
        "parseComponent",
        "compileTemplate",
        "compileScript",
        "compileStyle",
        "cssVars",
        "rewriteDefault",
    ] {
        write_text(
            &sfc_src.join(format!("{module}.ts")),
            "export * from 'vue/compiler-sfc'\n",
        )?;
    }
    write_text(
        &sfc_src.join("prefixIdentifiers.ts"),
        &vue27_sfc_prefix_identifiers_source_shim(),
    )?;
    Ok(())
}

fn vue27_sfc_prefix_identifiers_source_shim() -> String {
    let bridge_path = PathBuf::from("target")
        .join("debug")
        .join(if cfg!(windows) {
            "vuec_node_bridge.exe"
        } else {
            "vuec_node_bridge"
        });
    format!(
        r#"
import cp from 'node:child_process'
import path from 'node:path'

const bridgeBin = process.env.VUEC_NODE_BRIDGE || path.resolve(process.cwd(), {})

function callBridge(command, payload) {{
  const result = cp.spawnSync(bridgeBin, [command], {{
    input: JSON.stringify(payload || {{}}),
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024
  }})
  if (result.error) throw result.error
  if (result.status !== 0) {{
    const error = new Error(result.stderr || result.stdout || `vuec bridge command failed: ${{command}}`)
    ;(error as any).code = 'VUEC_BRIDGE_FAILED'
    throw error
  }}
  return result.stdout.trim() ? JSON.parse(result.stdout) : undefined
}}

export function prefixIdentifiers(source, isFunctional = false, isTS = false, babelOptions = {{}}, bindings) {{
  return callBridge('sfc.vue27.prefixIdentifiers', {{
    source: source == null ? '' : String(source),
    isFunctional: !!isFunctional,
    isTS: !!isTS,
    babelOptions: babelOptions || {{}},
    bindings: bindings || {{}}
  }})
}}
"#,
        js_string_literal(&bridge_path.to_string_lossy())
    )
}

fn write_vue27_compiler_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue2_vitest_setup(prepared_root)?;
    let config = r#"
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT

export default {
  define: {
    __DEV__: true,
    __TEST__: true,
  },
  resolve: {
    alias: {
      compiler: path.resolve(root, 'src/compiler'),
      core: path.resolve(root, 'src/core'),
      shared: path.resolve(root, 'src/shared'),
      web: path.resolve(root, 'src/platforms/web'),
      types: path.resolve(root, 'src/types'),
      vue: path.resolve(npmRoot, 'node_modules/vue/dist/vue.common.js'),
      vitest: path.resolve(npmRoot, 'node_modules/vitest/dist/index.js'),
      'vue-template-compiler': path.resolve(aliasRoot, 'node_modules/vue-template-compiler/index.js'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['test/unit/modules/compiler/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)
}

fn write_vue27_sfc_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue2_vitest_setup(prepared_root)?;
    let config = r#"
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT

export default {
  define: {
    __DEV__: true,
    __TEST__: true,
  },
  resolve: {
    alias: {
      compiler: path.resolve(root, 'src/compiler'),
      core: path.resolve(root, 'src/core'),
      shared: path.resolve(root, 'src/shared'),
      web: path.resolve(root, 'src/platforms/web'),
      types: path.resolve(root, 'src/types'),
      vitest: path.resolve(npmRoot, 'node_modules/vitest/dist/index.js'),
      'vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/vue/compiler-sfc/index.js'),
      vue: path.resolve(npmRoot, 'node_modules/vue/dist/vue.common.js'),
      'vue-template-compiler': path.resolve(aliasRoot, 'node_modules/vue-template-compiler/index.js'),
      '@babel/parser': path.resolve(npmRoot, 'node_modules/@babel/parser/lib/index.js'),
      postcss: path.resolve(npmRoot, 'node_modules/postcss/lib/postcss.mjs'),
      prettier: path.resolve(npmRoot, 'node_modules/prettier/index.js'),
      typescript: path.resolve(npmRoot, 'node_modules/typescript/lib/typescript.js'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-sfc/test/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)
}

fn write_vue2_vitest_setup(prepared_root: &Path) -> Result<()> {
    write_vuec_vitest_provenance_setup(prepared_root)?;
    write_text(
        &prepared_root.join("vuec-vitest-setup.ts"),
        r#"
import './vuec-vitest-provenance'
import { beforeEach, expect } from 'vitest'

const warnings: string[] = []
const warnMock = (...args: unknown[]) => {
  warnings.push(args.map(arg => String(arg)).join(' '))
}
;(warnMock as any).mock = { calls: [] as unknown[][] }

beforeEach(() => {
  warnings.length = 0
  ;(warnMock as any).mock.calls.length = 0
})

console.warn = (...args: unknown[]) => {
  ;(warnMock as any).mock.calls.push(args)
  warnMock(...args)
}
console.error = console.warn
;(console.error as any).mock = (warnMock as any).mock

expect.extend({
  toHaveBeenWarned(received) {
    const expected = String(received)
    const pass = warnings.some(warning => warning.includes(expected))
    return {
      pass,
      message: () => `expected ${JSON.stringify(expected)} ${pass ? 'not ' : ''}to have been warned`,
    }
  },
})
"#,
    )
}

fn write_vuec_vitest_provenance_setup(prepared_root: &Path) -> Result<()> {
    write_text(
        &prepared_root.join("vuec-vitest-provenance.ts"),
        r#"
import fs from 'node:fs'
import { afterEach, expect } from 'vitest'

const sidecarBase = process.env.VUEC_PROVENANCE_SIDECAR
const sidecarPath = sidecarBase ? `${sidecarBase}.${process.pid}.ndjson` : ''

function normalizePath(value: unknown): string {
  return String(value || '').replace(/\\/g, '/')
}

function flushVuecProvenance(): string[] {
  const flush = (globalThis as any).__vuecFlushProvenance
  if (typeof flush !== 'function') return []
  try {
    return flush().map((marker: unknown) => String(marker)).filter(Boolean)
  } catch {
    return []
  }
}

afterEach(() => {
  const markers = flushVuecProvenance()
  if (!sidecarPath || markers.length === 0) return
  const state = typeof expect.getState === 'function' ? expect.getState() : ({} as any)
  const record = {
    testPath: normalizePath((state as any).testPath),
    fullName: String((state as any).currentTestName || ''),
    title: String((state as any).currentTestName || ''),
    markers,
  }
  fs.appendFileSync(sidecarPath, `${JSON.stringify(record)}\n`)
})
"#,
    )
}

fn write_vue2_jasmine_runner(prepared_root: &Path) -> Result<()> {
    write_text(
        &prepared_root.join("vuec-jasmine-runner.js"),
        r#"
const fs = require('fs')
const path = require('path')
const Module = require('module')
const Jasmine = require('jasmine')
const { JSDOM } = require('jsdom')

const dom = new JSDOM('<!doctype html><html><body></body></html>')
global.window = dom.window
global.document = dom.window.document
global.navigator = dom.window.navigator

function vuecInteropDefault(value) {
  return value && Object.prototype.hasOwnProperty.call(value, 'default') ? value.default : value
}
globalThis.__vuecInteropDefault = vuecInteropDefault

require('@babel/register')({
  cache: false,
  extensions: ['.js', '.ts'],
  ignore: [/node_modules/],
  plugins: [
    function vuecModuleToCommonJs() {
      return {
        visitor: {
          ImportDeclaration(path) {
            const t = require('@babel/core').types
            const source = path.node.source
            const statements = []
            for (const spec of path.node.specifiers) {
              if (t.isImportDefaultSpecifier(spec)) {
                statements.push(t.variableDeclaration('const', [
                  t.variableDeclarator(spec.local, t.callExpression(t.memberExpression(t.identifier('globalThis'), t.identifier('__vuecInteropDefault')), [t.callExpression(t.identifier('require'), [source])])),
                ]))
              } else if (t.isImportNamespaceSpecifier(spec)) {
                statements.push(t.variableDeclaration('const', [
                  t.variableDeclarator(spec.local, t.callExpression(t.identifier('require'), [source])),
                ]))
              } else if (t.isImportSpecifier(spec)) {
                statements.push(t.variableDeclaration('const', [
                  t.variableDeclarator(
                    t.objectPattern([t.objectProperty(spec.imported, spec.local, false, spec.imported.name === spec.local.name)]),
                    t.callExpression(t.identifier('require'), [source])
                  ),
                ]))
              }
            }
            path.replaceWithMultiple(statements.length ? statements : [t.expressionStatement(t.callExpression(t.identifier('require'), [source]))])
          },
          ExportNamedDeclaration(path) {
            const t = require('@babel/core').types
            const node = path.node
            const statements = []
            if (node.declaration) {
              const decl = node.declaration
              statements.push(decl)
              if (t.isFunctionDeclaration(decl) || t.isClassDeclaration(decl)) {
                statements.push(t.expressionStatement(t.assignmentExpression('=', t.memberExpression(t.identifier('exports'), decl.id), decl.id)))
              } else if (t.isVariableDeclaration(decl)) {
                for (const d of decl.declarations) {
                  if (t.isIdentifier(d.id)) statements.push(t.expressionStatement(t.assignmentExpression('=', t.memberExpression(t.identifier('exports'), d.id), d.id)))
                }
              }
            }
            for (const spec of node.specifiers || []) {
              statements.push(t.expressionStatement(t.assignmentExpression('=', t.memberExpression(t.identifier('exports'), spec.exported), spec.local)))
            }
            path.replaceWithMultiple(statements)
          },
        },
      }
    },
  ],
})

const root = __dirname
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT
const reportPath = process.env.VUEC_JASMINE_REPORT || path.join(root, 'jasmine-report.json')
const originalResolve = Module._resolveFilename
Module._resolveFilename = function(request, parent, isMain, options) {
  const aliases = {
    compiler: path.join(root, 'src/compiler'),
    core: path.join(root, 'src/core'),
    shared: path.join(root, 'src/shared'),
    web: path.join(root, 'src/platforms/web'),
    types: path.join(root, 'src/types'),
    vue: path.join(npmRoot, 'node_modules/vue/dist/vue.common.js'),
    'vue-template-compiler': path.join(aliasRoot, 'node_modules/vue-template-compiler/index.js'),
  }
  for (const [key, target] of Object.entries(aliases)) {
    if (request === key) return originalResolve.call(this, target, parent, isMain, options)
    if (request.startsWith(key + '/')) {
      return originalResolve.call(this, path.join(target, request.slice(key.length + 1)), parent, isMain, options)
    }
  }
  return originalResolve.call(this, request, parent, isMain, options)
}

const warnings = []
global.__VUEC_WARNINGS__ = warnings
console.error = (...args) => {
  warnings.push(args.map(String).join(' '))
}
console.warn = console.error
console.error.calls = {
  count() {
    return warnings.length
  },
  argsFor(index) {
    const warning = warnings[index]
    return warning == null ? [] : [warning]
  },
}

fs.writeFileSync(path.join(root, 'vuec-jasmine-helper.js'), `
const warnings = global.__VUEC_WARNINGS__ || []
beforeEach(() => {
  warnings.length = 0
  jasmine.addMatchers({
    toHaveBeenWarned() {
      return {
        compare(actual) {
          const expected = String(actual)
          const pass = warnings.some(warning => warning.includes(expected))
          return {
            pass,
            message: pass
              ? 'expected ' + JSON.stringify(expected) + ' not to have been warned'
              : 'expected ' + JSON.stringify(expected) + ' to have been warned',
          }
        }
      }
    }
  })
})
`)

const jasmine = new Jasmine()
const specFiles = [
  'codeframe.spec.js',
  'codegen.spec.js',
  'compiler-options.spec.js',
  'optimizer.spec.js',
  'parser.spec.js',
].map(file => path.join(root, 'test/unit/modules/compiler', file))
jasmine.loadConfig({
  spec_dir: root,
  spec_files: [],
  helpers: [path.join(root, 'vuec-jasmine-helper.js')],
  random: false,
})
for (const file of specFiles) {
  jasmine.addSpecFile(file)
}

function normalizedPath(file) {
  return path.resolve(file).replace(/\\/g, '/')
}

const normalizedSpecFiles = specFiles.map(normalizedPath)
const specFileById = new Map()
const originalIt = global.it
global.it = function() {
  const stack = String(new Error().stack || '').replace(/\\/g, '/')
  const sourceFile = normalizedSpecFiles.find(file => stack.includes(file)) || '<unknown>'
  const spec = originalIt.apply(this, arguments)
  if (spec && spec.id) specFileById.set(spec.id, sourceFile)
  return spec
}

const testResultsByFile = new Map()
function fileResult(file) {
  if (!testResultsByFile.has(file)) {
    testResultsByFile.set(file, { name: file, assertionResults: [] })
  }
  return testResultsByFile.get(file)
}

function reportStatus(status) {
  if (status === 'passed') return 'passed'
  if (status === 'failed') return 'failed'
  if (status === 'pending' || status === 'disabled' || status === 'excluded') return 'skipped'
  return 'pending'
}

function flushVuecProvenance() {
  const flush = globalThis.__vuecFlushProvenance
  if (typeof flush !== 'function') return []
  try {
    return flush().map(marker => String(marker)).filter(Boolean)
  } catch {
    return []
  }
}

const counts = { total: 0, pass: 0, fail: 0, skip: 0, pending: 0 }
jasmine.addReporter({
  specDone(result) {
    counts.total += 1
    if (result.status === 'passed') counts.pass += 1
    else if (result.status === 'failed') counts.fail += 1
    else if (result.status === 'pending' || result.status === 'disabled' || result.status === 'excluded') counts.skip += 1
    else counts.pending += 1
    const sourceFile = specFileById.get(result.id) || '<unknown>'
    const coverageProvenance = flushVuecProvenance()
    const assertion = {
      title: result.fullName || result.description || '',
      status: reportStatus(result.status),
      failureMessages: (result.failedExpectations || []).map(expectation => expectation.message || '').filter(Boolean),
    }
    if (coverageProvenance.length) assertion.coverageProvenance = coverageProvenance
    fileResult(sourceFile).assertionResults.push(assertion)
  },
  jasmineDone() {
    counts.pending = Math.max(0, counts.total - counts.pass - counts.fail - counts.skip)
    fs.writeFileSync(reportPath, JSON.stringify({ counts, testResults: Array.from(testResultsByFile.values()) }, null, 2))
  },
})

jasmine.execute()
"#,
    )
}
