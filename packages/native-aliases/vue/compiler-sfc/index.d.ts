export function parse(source: string, options?: Record<string, unknown>): unknown;
export function parseComponent(source: string, options?: Record<string, unknown>): unknown;
export function compileTemplate(options: Record<string, unknown>): unknown;
export function compileScript(descriptor: unknown, options?: Record<string, unknown>): unknown;
export function compileStyle(options: Record<string, unknown>): unknown;
export function compileStyleAsync(options: Record<string, unknown>): Promise<unknown>;
export function rewriteDefault(source: string, variable: string, parserPlugins?: unknown): string;
export function generateCodeFrame(source: string, start?: number, end?: number): string;
