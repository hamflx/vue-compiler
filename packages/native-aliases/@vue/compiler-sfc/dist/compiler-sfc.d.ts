export class MagicString {
  constructor(original: string);
  append(content: string): this;
  prepend(content: string): this;
  overwrite(start: number, end: number, content: string): this;
  remove(start: number, end: number): this;
  slice(start: number, end?: number): string;
  toString(): string;
  generateMap(): unknown;
  static Bundle: unknown;
  static SourceMap: unknown;
  static default: typeof MagicString;
}

export function babelParse(source: string, options?: Record<string, unknown>): unknown;
export function parse(source: string, options?: Record<string, unknown>): unknown;
export function compileTemplate(options: Record<string, unknown>): unknown;
export function compileScript(descriptor: unknown, options?: Record<string, unknown>): unknown;
export function compileStyle(options: Record<string, unknown>): unknown;
export function compileStyleAsync(options: Record<string, unknown>): Promise<unknown>;
export function generateCodeFrame(source: string, start?: number, end?: number): string;
export function rewriteDefault(source: string, as: string, parserPlugins?: unknown): string;
export function rewriteDefaultAST(ast: unknown, s: MagicString, as: string): void;
export function extractIdentifiers(param: unknown): unknown[];
export function walkIdentifiers(root: unknown, onIdentifier: (...args: unknown[]) => void, includeAll?: boolean): void;
export function walk(root: unknown, enter?: (...args: unknown[]) => void, leave?: (...args: unknown[]) => void): unknown;
export function extractRuntimeProps(ctx: unknown): string | undefined;
export function extractRuntimeEmits(ctx: unknown): string | undefined;
export function inferRuntimeType(ctx: unknown, node: unknown): string[];
export function invalidateTypeCache(ctx?: unknown): void;
export function isInDestructureAssignment(parent: unknown, parentStack?: unknown[]): boolean;
export function isStaticProperty(node: unknown): boolean;
export function registerTS(ts: unknown): void;
export function resolveTypeElements(ctx: unknown, node: unknown, scope?: unknown, typeParameters?: unknown): unknown;
export function shouldTransformRef(): boolean;
export const errorMessages: Record<string, string>;
export const parseCache: Record<string, unknown>;
export const version: string;
