export * from '@vue/compiler-core';
export function compile(source: string, options?: Record<string, unknown>): unknown;
export function parse(source: string, options?: Record<string, unknown>): unknown;
export const parserOptions: Record<string, unknown>;
