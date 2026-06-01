export interface Vue2CompileResult {
  render: string;
  static_render_fns?: string[];
  staticRenderFns?: string[];
  errors: unknown[];
  tips: unknown[];
  diagnostics: string[];
}

export interface Vue3CodegenResult {
  code: string;
  map?: unknown;
  ast_summary?: string;
  astSummary?: string;
  diagnostics: unknown[];
  preamble: string;
}

export interface SfcDescriptor {
  filename: string;
  source: string;
  template: unknown;
  script: unknown;
  scriptSetup: unknown;
  styles: unknown[];
  customBlocks: unknown[];
  cssVars: string[];
  slotted: boolean;
  shouldForceReload?: unknown;
}

export interface SfcParseResult {
  descriptor: SfcDescriptor;
  errors: unknown[];
}

export function version(): string;
export function apiManifest(): {
  package: string;
  version: string;
  exports: string[];
};
export function bindingInfo(): {
  source: string;
  path: string;
  package: string | null;
  platform: string;
  arch: string;
};

export function compileVue2(template: string, options?: Record<string, unknown>): Vue2CompileResult;
export function compileToFunctionsVue2(template: string, options?: Record<string, unknown>): unknown;
export function compileSsrVue2(template: string, options?: Record<string, unknown>): Vue2CompileResult;
export function generateCodeFrameVue2(source: string, start?: number, end?: number): string;
export function callVue2Bridge(command: string, payload?: Record<string, unknown>): unknown;
export function rewriteDefaultVue27(source: string, variable: string, parserPlugins?: unknown): string;
export function rewriteDefaultVue3(source: string, variable: string, parserPlugins?: unknown): string;
export function baseCompileVue3(source: string, options?: Record<string, unknown>): Vue3CodegenResult;
export function baseParseVue3(source: string, options?: Record<string, unknown>): unknown;
export function generateVue3Core(ast: unknown, options?: Record<string, unknown>): Vue3CodegenResult;
export function callVue3CoreProjection(command: string, payload?: Record<string, unknown>): unknown;
export function callVue3DomProjection(command: string, payload?: Record<string, unknown>): unknown;
export function compileVue3Dom(source: string, options?: Record<string, unknown>): Vue3CodegenResult;
export function parseVue3Dom(source: string, options?: Record<string, unknown>): unknown;
export function compileVue3Ssr(source: string, options?: Record<string, unknown>): Vue3CodegenResult;
export function parseSfc(source: string, options?: Record<string, unknown>): SfcDescriptor;
export function parseSfcResult(source: string, options?: Record<string, unknown>): SfcParseResult;
export function parseVue27SfcComponent(source: string, options?: Record<string, unknown>): unknown;
export function compileSfcTemplate(source: string, options?: Record<string, unknown>): unknown;
export function compileSfcTemplateSource(source: string, options?: Record<string, unknown>): unknown;
export function compileSfcScript(source: string, options?: Record<string, unknown>): unknown;
export function compileVue27SfcTemplate(source: string, options?: Record<string, unknown>): unknown;
export function compileVue27SfcScript(source: string, options?: Record<string, unknown>): unknown;
export function compileSfcStyle(source: string, options?: Record<string, unknown>): unknown;

export function compile(template: string, options?: Record<string, unknown>): Vue2CompileResult;
export function compileToFunctions(template: string, options?: Record<string, unknown>): unknown;
export function baseCompile(source: string, options?: Record<string, unknown>): Vue3CodegenResult;
export function compileDom(source: string, options?: Record<string, unknown>): Vue3CodegenResult;
export function compileSsr(source: string, options?: Record<string, unknown>): Vue3CodegenResult;
export function parse(source: string, options?: Record<string, unknown>): SfcDescriptor;
export function compileTemplate(options: Record<string, unknown> & { source: string }): unknown;
export function compileScript(descriptor: SfcDescriptor, options?: Record<string, unknown>): unknown;
export function compileStyle(options: Record<string, unknown> & { source: string }): unknown;
