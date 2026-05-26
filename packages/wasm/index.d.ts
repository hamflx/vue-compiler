export interface WasmInitOptions {
  source?: string;
}

export interface CompileOptions {
  filename?: string;
  mode?: 'function' | 'module';
  prefixIdentifiers?: boolean;
  sourceMap?: boolean;
  scopeId?: string;
  id?: string;
  ssr?: boolean;
  scoped?: boolean;
  slotted?: boolean;
  isProd?: boolean;
  inlineTemplate?: boolean;
  preprocessLang?: string;
  vars?: string[];
  [key: string]: unknown;
}

export function init(input?: string): Promise<typeof api>;
export function version(): string;
export function compileVue2(template: string, options?: CompileOptions): unknown;
export function compileVue3Dom(source: string, options?: CompileOptions): unknown;
export function compileVue3Ssr(source: string, options?: CompileOptions): unknown;
export function parseSfc(source: string, options?: CompileOptions): unknown;
export function compileSfcTemplate(source: string, options?: CompileOptions): unknown;
export function compileSfcTemplateSource(source: string, options?: CompileOptions): unknown;
export function compileSfcScript(source: string, options?: CompileOptions): unknown;
export function compileSfcStyle(source: string, options?: CompileOptions): unknown;
export function compile(template: string, options?: CompileOptions): unknown;
export function compileDom(source: string, options?: CompileOptions): unknown;
export function compileSsr(source: string, options?: CompileOptions): unknown;
export function parse(source: string, options?: CompileOptions): unknown;
export function compileTemplate(options?: CompileOptions & { source?: string }): unknown;
export function compileScript(descriptor: { source?: string; filename?: string }, options?: CompileOptions): unknown;
export function compileStyle(options?: CompileOptions & { source?: string; lang?: string }): unknown;

export const api: {
  init: typeof init;
  version: typeof version;
  compileVue2: typeof compileVue2;
  compileVue3Dom: typeof compileVue3Dom;
  compileVue3Ssr: typeof compileVue3Ssr;
  parseSfc: typeof parseSfc;
  compileSfcTemplate: typeof compileSfcTemplate;
  compileSfcTemplateSource: typeof compileSfcTemplateSource;
  compileSfcScript: typeof compileSfcScript;
  compileSfcStyle: typeof compileSfcStyle;
  compile: typeof compile;
  compileDom: typeof compileDom;
  compileSsr: typeof compileSsr;
  parse: typeof parse;
  compileTemplate: typeof compileTemplate;
  compileScript: typeof compileScript;
  compileStyle: typeof compileStyle;
};
