export function compile(template: string, options?: Record<string, unknown>): unknown;
export function compileToFunctions(template: string, options?: Record<string, unknown>, vm?: unknown): unknown;
export function ssrCompile(template: string, options?: Record<string, unknown>): unknown;
export function ssrCompileToFunctions(template: string, options?: Record<string, unknown>, vm?: unknown): unknown;
export function parseComponent(source: string, options?: Record<string, unknown>): unknown;
export function generateCodeFrame(source: string, start?: number, end?: number): string;
