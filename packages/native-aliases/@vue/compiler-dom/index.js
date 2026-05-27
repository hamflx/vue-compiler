'use strict';

const core = require('@vue/compiler-core');
const native = require('@vuec-rs/native');

const helperNameMap = core.helperNameMap;

const TRANSITION = registerHelper('Transition', 'Transition');
const TRANSITION_GROUP = registerHelper('TransitionGroup', 'TransitionGroup');
const V_MODEL_RADIO = registerHelper('vModelRadio', 'vModelRadio');
const V_MODEL_CHECKBOX = registerHelper('vModelCheckbox', 'vModelCheckbox');
const V_MODEL_TEXT = registerHelper('vModelText', 'vModelText');
const V_MODEL_SELECT = registerHelper('vModelSelect', 'vModelSelect');
const V_MODEL_DYNAMIC = registerHelper('vModelDynamic', 'vModelDynamic');
const V_ON_WITH_MODIFIERS = registerHelper('vOnModifiersGuard', 'withModifiers');
const V_ON_WITH_KEYS = registerHelper('vOnKeysGuard', 'withKeys');
const V_SHOW = registerHelper('vShow', 'vShow');

const DOMErrorCodes = enumObject(54, [
  'X_V_HTML_NO_EXPRESSION',
  'X_V_HTML_WITH_CHILDREN',
  'X_V_TEXT_NO_EXPRESSION',
  'X_V_TEXT_WITH_CHILDREN',
  'X_V_MODEL_ON_INVALID_ELEMENT',
  'X_V_MODEL_ARG_ON_ELEMENT',
  'X_V_MODEL_ON_FILE_INPUT_ELEMENT',
  'X_V_MODEL_UNNECESSARY_VALUE',
  'X_V_SHOW_NO_EXPRESSION',
  'X_TRANSITION_INVALID_CHILDREN',
  'X_IGNORED_SIDE_EFFECT_TAG',
  '__EXTEND_POINT__',
]);

const DOMErrorMessages = {
  54: 'v-html is missing expression.',
  55: 'v-html will override element children.',
  56: 'v-text is missing expression.',
  57: 'v-text will override element children.',
  58: 'v-model can only be used on <input>, <textarea> and <select> elements.',
  59: 'v-model argument is not supported on plain elements.',
  60: 'v-model cannot be used on file inputs since they are read-only. Use a v-on:change listener instead.',
  61: "Unnecessary value binding used alongside v-model. It will interfere with v-model's behavior.",
  62: 'v-show is missing expression.',
  63: '<Transition> expects exactly one child element or component.',
  64: 'Tags with side effect (<script> and <style>) are ignored in client component templates.',
};

function callVue3DomProjection(command, payload) {
  return native.callVue3DomProjection(command, payload || {});
}

const transformStyle = (node) => {
  if (!node || node.type !== core.NodeTypes.ELEMENT) return undefined;
  const projection = callVue3DomProjection('vue3.dom.transformStyle', { node });
  for (const replacement of projection && projection.replacements || []) {
    const original = node.props && node.props[replacement.index];
    if (!original || original.type !== core.NodeTypes.ATTRIBUTE) continue;
    node.props[replacement.index] = {
      type: core.NodeTypes.DIRECTIVE,
      name: 'bind',
      rawName: ':style',
      arg: core.createSimpleExpression('style', true, original.loc),
      exp: core.createSimpleExpression(
        replacement.expression || '{}',
        false,
        original.loc,
        core.ConstantTypes.CAN_STRINGIFY,
      ),
      modifiers: [],
      loc: original.loc,
    };
  }
  return undefined;
};

const DOMNodeTransforms = [
  transformStyle,
  function transformTransition(node, context) {
    return undefined;
  },
  function validateHtmlNesting(node, context) {
    return undefined;
  },
];

const DOMDirectiveTransforms = {
  cloak: core.noopDirectiveTransform,
  html: function transformVHtml(dir, node, context) {
    return dir && dir.exp
      ? { props: [core.createObjectProperty('innerHTML', dir.exp)] }
      : { props: [] };
  },
  text: function transformVText(dir, node, context) {
    return dir && dir.exp
      ? { props: [core.createObjectProperty('textContent', dir.exp)] }
      : { props: [] };
  },
  model: function transformModel(dir, node, context) {
    return core.transformModel(dir, node, context);
  },
  on: function transformOn(dir, node, context) {
    return core.transformOn(dir, node, context);
  },
  show: function transformShow(dir, node, context) {
    return { props: [], needRuntime: V_SHOW };
  },
};

const parserOptions = {
  parseMode: 'html',
  isVoidTag(tag) {
    return /^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(String(tag || ''));
  },
  isNativeTag(tag) {
    const name = String(tag || '');
    return isHtmlTag(name) || isSvgTag(name) || isMathMlTag(name);
  },
  isPreTag(tag) {
    return String(tag || '').toLowerCase() === 'pre';
  },
  isIgnoreNewlineTag(tag) {
    return /^(textarea|pre)$/i.test(String(tag || ''));
  },
  isBuiltInComponent(tag) {
    const name = String(tag || '');
    if (/^transition$/i.test(name)) return TRANSITION;
    if (/^transition-group$/i.test(name) || /^transitiongroup$/i.test(name)) return TRANSITION_GROUP;
    return undefined;
  },
  decodeEntities(rawText, asAttr) {
    return String(rawText || '')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&amp;/g, '&')
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'");
  },
  getNamespace(tag, parent, rootNamespace) {
    const name = String(tag || '');
    if (name === 'svg') return 1;
    if (name === 'math') return 2;
    if (parent && parent.ns === 1 && /^(foreignObject|desc|title)$/.test(parent.tag || '')) return 0;
    return rootNamespace == null ? 0 : rootNamespace;
  },
};

function compile(src) {
  return native.compileVue3Dom(String(src || ''), arguments[1] || {});
}

function parse(template) {
  return native.parseVue3Dom(String(template || ''), arguments[1] || {});
}

function createDOMCompilerError(code, loc) {
  return core.createCompilerError(code, loc, DOMErrorMessages);
}

function registerHelper(symbolName, runtimeName) {
  const symbol = Symbol(symbolName);
  helperNameMap[symbol] = runtimeName;
  return symbol;
}

function enumObject(start, names) {
  const out = {};
  names.forEach((name, index) => {
    const value = start + index;
    out[value] = name;
    out[name] = value;
  });
  return out;
}

function isHtmlTag(tag) {
  return HTML_TAGS.has(tag.toLowerCase());
}

function isSvgTag(tag) {
  return SVG_TAGS.has(tag);
}

function isMathMlTag(tag) {
  return MATH_ML_TAGS.has(tag);
}

const HTML_TAGS = new Set(
  'html,body,base,head,link,meta,style,title,address,article,aside,footer,header,hgroup,h1,h2,h3,h4,h5,h6,nav,section,div,dd,dl,dt,figcaption,figure,picture,hr,img,li,main,ol,p,pre,ul,a,b,abbr,bdi,bdo,br,cite,code,data,dfn,em,i,kbd,mark,q,rp,rt,ruby,s,samp,small,span,strong,sub,sup,time,u,var,wbr,area,audio,map,track,video,embed,object,param,source,canvas,script,noscript,del,ins,caption,col,colgroup,table,thead,tbody,td,th,tr,button,datalist,fieldset,form,input,label,legend,meter,optgroup,option,output,progress,select,textarea,details,dialog,menu,summary,template,blockquote,iframe,tfoot'.split(','),
);

const SVG_TAGS = new Set(
  'svg,animate,animateMotion,animateTransform,circle,clipPath,color-profile,defs,desc,discard,ellipse,feBlend,feColorMatrix,feComponentTransfer,feComposite,feConvolveMatrix,feDiffuseLighting,feDisplacementMap,feDistantLight,feDropShadow,feFlood,feFuncA,feFuncB,feFuncG,feFuncR,feGaussianBlur,feImage,feMerge,feMergeNode,feMorphology,feOffset,fePointLight,feSpecularLighting,feSpotLight,feTile,feTurbulence,filter,foreignObject,g,hatch,hatchpath,image,line,linearGradient,marker,mask,mesh,meshgradient,meshpatch,meshrow,metadata,mpath,path,pattern,polygon,polyline,radialGradient,rect,set,solidcolor,stop,switch,symbol,text,textPath,title,tspan,unknown,use,view'.split(','),
);

const MATH_ML_TAGS = new Set(
  'math,maction,maligngroup,malignmark,menclose,merror,mfenced,mfrac,mi,mlabeledtr,mlongdiv,mmultiscripts,mn,mo,mover,mpadded,mphantom,ms,mspace,msqrt,mstyle,msub,msup,msubsup,mtable,mtd,mtext,mtr,munder,munderover,none,semantics'.split(','),
);

module.exports = {
  ...core,
  DOMDirectiveTransforms,
  DOMErrorCodes,
  DOMErrorMessages,
  DOMNodeTransforms,
  TRANSITION,
  TRANSITION_GROUP,
  V_MODEL_CHECKBOX,
  V_MODEL_DYNAMIC,
  V_MODEL_RADIO,
  V_MODEL_SELECT,
  V_MODEL_TEXT,
  V_ON_WITH_KEYS,
  V_ON_WITH_MODIFIERS,
  V_SHOW,
  compile,
  createDOMCompilerError,
  parse,
  parserOptions,
  transformStyle,
};
