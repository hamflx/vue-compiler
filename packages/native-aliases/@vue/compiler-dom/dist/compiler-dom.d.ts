export * from '@vue/compiler-core';

export type CompilerDomCompatValue = unknown;

export const DOMDirectiveTransforms: Record<string, CompilerDomCompatValue>;
export const DOMErrorCodes: Record<string, string | number>;
export const DOMErrorMessages: Record<string, string>;
export const DOMNodeTransforms: CompilerDomCompatValue[];
export const TRANSITION: symbol;
export const TRANSITION_GROUP: symbol;
export const V_MODEL_CHECKBOX: symbol;
export const V_MODEL_DYNAMIC: symbol;
export const V_MODEL_RADIO: symbol;
export const V_MODEL_SELECT: symbol;
export const V_MODEL_TEXT: symbol;
export const V_ON_WITH_KEYS: symbol;
export const V_ON_WITH_MODIFIERS: symbol;
export const V_SHOW: symbol;
export const parserOptions: {
  parseMode: string;
  isVoidTag(tag: string): boolean;
  isNativeTag(tag: string): boolean;
  isPreTag(tag: string): boolean;
  isIgnoreNewlineTag(tag: string): boolean;
  isBuiltInComponent(tag: string): symbol | undefined;
  decodeEntities(rawText: string, asAttr?: boolean): string;
  getNamespace(tag: string, parent?: CompilerDomCompatValue, rootNamespace?: number): number;
};

export function compile(src: string, options?: Record<string, unknown>): CompilerDomCompatValue;
export function createDOMCompilerError(code: CompilerDomCompatValue, loc?: CompilerDomCompatValue): Error;
export function parse(template: string, options?: Record<string, unknown>): CompilerDomCompatValue;
export function transformStyle(node: CompilerDomCompatValue): CompilerDomCompatValue;
