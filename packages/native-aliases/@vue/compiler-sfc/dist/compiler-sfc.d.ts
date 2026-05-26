export function parse(source: string, options?: Record<string, unknown>): unknown;
export function compileTemplate(options: Record<string, unknown>): unknown;
export function compileScript(descriptor: unknown, options?: Record<string, unknown>): unknown;
export function compileStyle(options: Record<string, unknown>): unknown;
export function compileStyleAsync(options: Record<string, unknown>): Promise<unknown>;
export function generateCodeFrame(source: string, start?: number, end?: number): string;
export const version: string;
