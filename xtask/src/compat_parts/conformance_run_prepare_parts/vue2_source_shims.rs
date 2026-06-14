fn write_vue2_compiler_source_shims(prepared_root: &Path, include_types: bool) -> Result<()> {
    let compiler_root = prepared_root.join("src").join("compiler");
    let parser_root = compiler_root.join("parser");
    fs::create_dir_all(&parser_root)
        .with_context(|| format!("failed to create {}", parser_root.display()))?;
    write_text(
        &parser_root.join("index.ts"),
        r#"
import { compile } from 'vue-template-compiler'

export function parse(template, options = {}) {
  const compiled = compile(template, vue2ParseBridgeOptions(options, template))
  const ast = compiled.element_public_ast || compiled.ast_public || compiled.ast || null
  if (ast && typeof ast === 'object') {
    Object.defineProperty(ast, '__vuecTemplate', { value: template, enumerable: false, configurable: true })
    Object.defineProperty(ast, '__vuecOptions', { value: options, enumerable: false, configurable: true })
    Object.defineProperty(ast, '__vuecInternal', { value: compiled.element_ast || null, enumerable: false, configurable: true })
    hydrateVue2PublicAst(ast, null, compiled.element_ast || null)
    runVue2ModuleTransforms(ast, options, 'preTransformNode')
    runVue2ModuleTransforms(ast, options, 'postTransformNode')
  }
  return ast
}

function vue2ParseBridgeOptions(options, template) {
  const hasMustUseProp = options && Object.prototype.hasOwnProperty.call(options, 'mustUseProp')
  const tags = extractVue2TemplateTags(template)
  return {
    ...normalizeVue2OptionsForBridge(options, tags, true),
    optimize: true,
    __vuecDisableDefaultMustUseProp: !hasMustUseProp,
    __vuecSuppressWarnings: ['Inline-template components must have exactly one child element.'],
  }
}

function normalizeVue2OptionsForBridge(options, tags, disableMissingPlatformOptions) {
  const normalized = {}
  if (options && typeof options === 'object') {
    for (const key of Object.keys(options)) {
      if (typeof options[key] !== 'function') normalized[key] = options[key]
    }
  }
  if (hasVue2PredicateOption(options, 'getTagNamespace')) {
    normalized.__vuecTagNamespaces = collectVue2NamespaceHits(options.getTagNamespace, tags)
    normalized.__vuecUseDefaultTagNamespaces = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecTagNamespaces = {}
    normalized.__vuecUseDefaultTagNamespaces = false
  }
  if (hasVue2PredicateOption(options, 'isReservedTag')) {
    normalized.__vuecReservedTags = collectVue2PredicateHits(options.isReservedTag, tags)
    normalized.__vuecUseDefaultReservedTags = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecReservedTags = []
    normalized.__vuecUseDefaultReservedTags = false
  }
  return normalized
}

function hasVue2PredicateOption(options, name) {
  return !!(options && Object.prototype.hasOwnProperty.call(options, name) &&
    (typeof options[name] === 'function' || Array.isArray(options[name])))
}

function extractVue2TemplateTags(source) {
  const tags = []
  const seen = new Set()
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g
  let match
  while ((match = pattern.exec(String(source || '')))) {
    const tag = match[1]
    if (!seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  }
  return tags
}

function collectVue2PredicateHits(predicate, values) {
  if (Array.isArray(predicate)) return predicate.map(String)
  if (typeof predicate !== 'function') return []
  const hits = []
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value)
    } catch (_) {}
  }
  return hits
}

function collectVue2NamespaceHits(getNamespace, values) {
  if (typeof getNamespace !== 'function') return {}
  const namespaces = {}
  for (const value of values) {
    try {
      const namespace = getNamespace(value)
      if (namespace !== undefined && namespace !== null) namespaces[value] = String(namespace)
    } catch (_) {}
  }
  return namespaces
}

function runVue2ModuleTransforms(ast, options, hook) {
  if (!ast || !options || !Array.isArray(options.modules)) return
  walkVue2PublicElements(ast, element => {
    for (const module of options.modules) {
      const transform = module && module[hook]
      if (typeof transform === 'function') transform(element, options)
    }
  })
}

function walkVue2PublicElements(node, visit) {
  if (!node || typeof node !== 'object' || typeof node.tag !== 'string') return
  visit(node)
  if (Array.isArray(node.children)) {
    for (const child of node.children) walkVue2PublicElements(child, visit)
  }
  if (node.scopedSlots && typeof node.scopedSlots === 'object') {
    for (const slot of Object.values(node.scopedSlots)) walkVue2PublicElements(slot, visit)
  }
}

function hydrateVue2PublicAst(node, parent, internal) {
  if (!node || typeof node !== 'object') return node
  if (parent) {
    Object.defineProperty(node, 'parent', { value: parent, enumerable: false, configurable: true, writable: true })
  }
  if (internal) {
    Object.defineProperty(node, '__vuecInternal', { value: internal, enumerable: false, configurable: true })
  }
  const internalChildren = Array.isArray(internal && internal.children) ? internal.children : []
  if (Array.isArray(node.children)) {
    node.children.forEach((child, index) => {
      const internalChild = internalChildren[index]
      hydrateVue2PublicAst(child, node, internalChild && (internalChild.Element || internalChild.Text))
    })
  }
  const internalConditions = Array.isArray(internal && internal.if_conditions) ? internal.if_conditions : []
  if (Array.isArray(node.ifConditions)) {
    node.ifConditions.forEach((condition, index) => {
      hydrateVue2PublicAst(condition && condition.block, parent, internalConditions[index] && internalConditions[index].block)
    })
  }
  const internalSlots = internal && internal.scoped_slots && typeof internal.scoped_slots === 'object'
    ? internal.scoped_slots
    : {}
  if (node.scopedSlots && typeof node.scopedSlots === 'object') {
    for (const [name, slot] of Object.entries(node.scopedSlots)) {
      hydrateVue2PublicAst(slot, node, internalSlots[name] || internalSlots[`"${name}"`] || null)
    }
  }
  return node
}
"#,
    )?;
    write_text(
        &compiler_root.join("optimizer.ts"),
        r#"
import * as vueTemplateCompiler from 'vue-template-compiler'
import { normalizeVue2AstForBridge } from './codegen'

export function optimize(ast, options = {}) {
  if (!ast) return ast
  const optimized = vueTemplateCompiler.__vuecRuntime.callBridge('vue2.optimize', {
    ast: normalizeVue2AstForBridge(ast),
    options: vue2OptimizeBridgeOptions(ast, options),
  })
  mergeVue2OptimizedAst(ast, optimized && (optimized.element_public_ast || optimized.ast_public || optimized.ast), optimized && optimized.element_ast)
  return ast
}

function vue2OptimizeBridgeOptions(ast, options) {
  const tags = collectVue2AstTags(ast)
  return normalizeVue2OptionsForBridge(options, tags, true)
}

function normalizeVue2OptionsForBridge(options, tags, disableMissingPlatformOptions) {
  const normalized = {}
  if (options && typeof options === 'object') {
    for (const key of Object.keys(options)) {
      if (typeof options[key] !== 'function') normalized[key] = options[key]
    }
  }
  if (hasVue2PredicateOption(options, 'getTagNamespace')) {
    normalized.__vuecTagNamespaces = collectVue2NamespaceHits(options.getTagNamespace, tags)
    normalized.__vuecUseDefaultTagNamespaces = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecTagNamespaces = {}
    normalized.__vuecUseDefaultTagNamespaces = false
  }
  if (hasVue2PredicateOption(options, 'isReservedTag')) {
    normalized.__vuecReservedTags = collectVue2PredicateHits(options.isReservedTag, tags)
    normalized.__vuecUseDefaultReservedTags = false
  } else if (disableMissingPlatformOptions) {
    normalized.__vuecReservedTags = []
    normalized.__vuecUseDefaultReservedTags = false
  }
  return normalized
}

function hasVue2PredicateOption(options, name) {
  return !!(options && Object.prototype.hasOwnProperty.call(options, name) &&
    (typeof options[name] === 'function' || Array.isArray(options[name])))
}

function collectVue2PredicateHits(predicate, values) {
  if (Array.isArray(predicate)) return predicate.map(String)
  if (typeof predicate !== 'function') return []
  const hits = []
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value)
    } catch (_) {}
  }
  return hits
}

function collectVue2NamespaceHits(getNamespace, values) {
  if (typeof getNamespace !== 'function') return {}
  const namespaces = {}
  for (const value of values) {
    try {
      const namespace = getNamespace(value)
      if (namespace !== undefined && namespace !== null) namespaces[value] = String(namespace)
    } catch (_) {}
  }
  return namespaces
}

function collectVue2AstTags(ast) {
  const tags = []
  const seen = new Set()
  walkVue2AstElements(ast, element => {
    const tag = String(element.tag || '')
    if (tag && !seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  })
  return tags
}

function walkVue2AstElements(node, visit) {
  if (!node || typeof node !== 'object') return
  if ('Element' in node) return walkVue2AstElements(node.Element, visit)
  if (typeof node.tag === 'string') {
    visit(node)
    if (Array.isArray(node.children)) {
      for (const child of node.children) walkVue2AstElements(child && (child.Element || child), visit)
    }
    const conditions = node.ifConditions || node.if_conditions
    if (Array.isArray(conditions)) {
      for (const condition of conditions) walkVue2AstElements(condition && condition.block, visit)
    }
    const scopedSlots = node.scopedSlots || node.scoped_slots
    if (scopedSlots && typeof scopedSlots === 'object') {
      for (const slot of Object.values(scopedSlots)) walkVue2AstElements(slot, visit)
    }
  }
}

function mergeVue2OptimizedAst(target, publicNode, internalNode) {
  if (!target || typeof target !== 'object') return
  if (internalNode) {
    Object.defineProperty(target, '__vuecInternal', { value: internalNode, enumerable: false, configurable: true })
  }
  if (publicNode && typeof publicNode === 'object') {
    target.static = Boolean(publicNode.static)
    target.staticRoot = Boolean(publicNode.staticRoot)
    target.staticInFor = Boolean(publicNode.staticInFor)
  } else if (internalNode && typeof internalNode === 'object') {
    target.static_node = Boolean(internalNode.static_node)
    target.static_root = Boolean(internalNode.static_root)
    target.static_in_for = Boolean(internalNode.static_in_for)
  }
  const targetChildren = Array.isArray(target.children) ? target.children : []
  const publicChildren = Array.isArray(publicNode && publicNode.children) ? publicNode.children : []
  const internalChildren = Array.isArray(internalNode && internalNode.children) ? internalNode.children : []
  targetChildren.forEach((child, index) => {
    const internalChild = internalChildren[index]
    mergeVue2OptimizedAst(child && (child.Element || child), publicChildren[index], internalChild && (internalChild.Element || internalChild.Text))
  })
  const targetConditions = target.ifConditions || target.if_conditions
  const publicConditions = publicNode && (publicNode.ifConditions || publicNode.if_conditions)
  const internalConditions = internalNode && internalNode.if_conditions
  if (Array.isArray(targetConditions)) {
    targetConditions.forEach((condition, index) => {
      mergeVue2OptimizedAst(
        condition && condition.block,
        publicConditions && publicConditions[index] && publicConditions[index].block,
        internalConditions && internalConditions[index] && internalConditions[index].block,
      )
    })
  }
  const targetSlots = target.scopedSlots || target.scoped_slots
  const publicSlots = publicNode && (publicNode.scopedSlots || publicNode.scoped_slots)
  const internalSlots = internalNode && internalNode.scoped_slots
  if (targetSlots && typeof targetSlots === 'object') {
    for (const [key, slot] of Object.entries(targetSlots)) {
      mergeVue2OptimizedAst(slot, publicSlots && (publicSlots[key] || publicSlots[`"${key}"`]), internalSlots && (internalSlots[key] || internalSlots[`"${key}"`]))
    }
  }
}
"#,
    )?;
    write_text(
        &compiler_root.join("codegen.ts"),
        r#"
import * as vueTemplateCompiler from 'vue-template-compiler'

export function generate(ast, options = {}) {
  const generated = vueTemplateCompiler.__vuecRuntime.callBridge('vue2.generate', {
    ast: normalizeVue2AstForBridge(ast),
    options,
  })
  emitVue2InlineTemplateWarnings(ast)
  return {
    render: generated.render,
    staticRenderFns: generated.staticRenderFns || generated.static_render_fns || [],
  }
}

export function normalizeVue2AstForBridge(node) {
  if (!node || typeof node !== 'object') return null
  if ('Element' in node) return normalizeVue2AstForBridge(node.Element)
  if (isInternalVue2ElementAst(node)) return normalizeVue2InternalAstForBridge(node)
  return normalizeVue2PublicElementForBridge(node)
}

function isInternalVue2ElementAst(node) {
  return !!(node && typeof node === 'object' && (
    Object.prototype.hasOwnProperty.call(node, 'attrs_list') ||
    Object.prototype.hasOwnProperty.call(node, 'static_node') ||
    Object.prototype.hasOwnProperty.call(node, 'if_conditions')
  ))
}

function normalizeVue2InternalAstForBridge(node) {
  if (!node || typeof node !== 'object') return null
  const copy = {}
  for (const key of Object.keys(node)) {
    if (key === 'parent' || key.startsWith('__vuec')) continue
    copy[key] = node[key]
  }
  normalizeVue2InternalEventsForBridge(copy.events)
  normalizeVue2InternalEventsForBridge(copy.native_events)
  if (Array.isArray(copy.children)) {
    copy.children = copy.children.map(normalizeVue2InternalNodeForBridge)
  }
  if (copy.scoped_slots && typeof copy.scoped_slots === 'object') {
    copy.scoped_slots = Object.fromEntries(
      Object.entries(copy.scoped_slots).map(([key, value]) => [key, normalizeVue2AstForBridge(value)])
    )
  }
  if (Array.isArray(copy.if_conditions)) {
    copy.if_conditions = copy.if_conditions.map(condition => ({
      ...condition,
      block: normalizeVue2AstForBridge(condition && condition.block),
    }))
  }
  return copy
}

function normalizeVue2InternalNodeForBridge(node) {
  if (!node || typeof node !== 'object') return node
  if ('Element' in node) return { Element: normalizeVue2AstForBridge(node.Element) }
  if ('Text' in node) return { Text: normalizeVue2TextForBridge(node.Text) }
  return normalizeVue2PublicNodeForBridge(node)
}

function normalizeVue2PublicElementForBridge(node) {
  const scopedSlots = normalizeVue2ScopedSlotsForBridge(node.scopedSlots || node.scoped_slots)
  const directives = normalizeVue2DirectivesForBridge(node.directives)
  const model = normalizeVue2ElementModelForBridge(node)
  const domModel = model && !isVue2ComponentElementForBridge(node)
  if (domModel) {
    directives.push(model)
  }
  const props = normalizeVue2AttrsForBridge(node.props)
  const events = normalizeVue2EventsForBridge(node.events)
  if (domModel) applyVue2DomModelForBridge(node, model, props, events)
  return {
    tag: String(node.tag || ''),
    attrs_list: normalizeVue2RawAttrsForBridge(node.attrsList || node.attrs_list),
    raw_attrs_list: normalizeVue2RawAttrsForBridge(node.attrsList || node.raw_attrs_list || node.attrs_list),
    attrs_map: normalizeVue2AttrsMapForBridge(node.attrsMap || node.attrs_map, node.attrsList || node.attrs_list),
    raw_attrs_map: normalizeVue2RawAttrsMapForBridge(node.rawAttrsMap || node.raw_attrs_map, node.attrsList || node.attrs_list),
    attrs: normalizeVue2AttrsForBridge(node.attrs),
    props,
    dynamic_attrs: normalizeVue2AttrsForBridge(node.dynamicAttrs || node.dynamic_attrs),
    directives,
    events,
    native_events: normalizeVue2EventsForBridge(node.nativeEvents || node.native_events),
    children: Array.isArray(node.children) ? node.children.map(normalizeVue2PublicNodeForBridge) : [],
    ns: node.ns,
    plain: Boolean(node.plain),
    forbidden: Boolean(node.forbidden),
    pre: Boolean(node.pre),
    once: Boolean(node.once),
    has_bindings: Boolean(node.hasBindings || node.has_bindings),
    if_exp: node.if ?? node.if_exp,
    elseif: node.elseif,
    else_branch: Boolean(node.else || node.else_branch),
    if_conditions: Array.isArray(node.ifConditions || node.if_conditions)
      ? (node.ifConditions || node.if_conditions).map(condition => ({
          exp: condition && condition.exp,
          block: normalizeVue2AstForBridge(condition && condition.block),
        }))
      : [],
    for_exp: node.for ?? node.for_exp,
    alias: node.alias,
    iterator1: node.iterator1,
    iterator2: node.iterator2,
    key: node.key,
    ref_name: node.ref ?? node.ref_name,
    ref_in_for: Boolean(node.refInFor || node.ref_in_for),
    slot_name: node.slotName ?? node.slot_name,
    slot_target: node.slotTarget ?? node.slot_target,
    slot_target_dynamic: Boolean(node.slotTargetDynamic || node.slot_target_dynamic),
    slot_scope: node.slotScope ?? node.slot_scope,
    slot_new_syntax: Boolean(node.slotNewSyntax || node.slot_new_syntax),
    scoped_slots: scopedSlots,
    component: node.component,
    inline_template: Boolean(node.inlineTemplate || node.inline_template),
    static_class: node.staticClass ?? node.static_class,
    class_binding: node.classBinding ?? node.class_binding,
    static_style: node.staticStyle ?? node.static_style,
    style_binding: node.styleBinding ?? node.style_binding,
    model: model && !domModel ? node.model : undefined,
    wrap_data: node.wrapData ?? node.wrap_data,
    wrap_listeners: node.wrapListeners ?? node.wrap_listeners,
    validate: node.validate,
    validators: Array.isArray(node.validators) ? node.validators : [],
    static_node: Boolean(node.static ?? node.static_node),
    static_root: Boolean(node.staticRoot ?? node.static_root),
    static_in_for: Boolean(node.staticInFor ?? node.static_in_for),
  }
}

function applyVue2DomModelForBridge(node, model, props, events) {
  const tag = String(node && node.tag || '')
  if (tag !== 'input' && tag !== 'textarea') return
  const attrsMap = node.attrsMap || node.attrs_map || {}
  const type = String(attrsMap.type || '').toLowerCase()
  if (type === 'checkbox' || type === 'radio') return
  const modifiers = vue2ModelModifiersForBridge(model.raw_name || model.rawName)
  const value = vue2ModelExpressionForBridge(model)
  props.push({ name: 'value', value: `(${value})`, dynamic: false })
  let assignmentValue = modifiers.trim ? '$event.target.value.trim()' : '$event.target.value'
  if (modifiers.number) assignmentValue = `_n(${assignmentValue})`
  const event = modifiers.lazy ? 'change' : type === 'range' ? '__r' : 'input'
  const guard = !modifiers.lazy && type !== 'range' ? 'if($event.target.composing)return;' : ''
  const handler = {
    value: `${guard}${vue2AssignmentCodeForBridge(value, assignmentValue)}`,
    modifiers: {},
    modifier_order: [],
    has_modifier_object: false,
    dynamic: false,
  }
  if (Array.isArray(events[event])) {
    events[event].unshift(handler)
  } else {
    events[event] = [handler]
  }
  if (modifiers.trim || modifiers.number) {
    events.blur = events.blur || []
    events.blur.push({
      value: 'return $forceUpdate()',
      modifiers: {},
      modifier_order: [],
      has_modifier_object: false,
      dynamic: false,
    })
  }
}

function vue2ModelModifiersForBridge(rawName) {
  const modifiers = {}
  for (const part of String(rawName || '').split('.').slice(1)) {
    if (part) modifiers[part] = true
  }
  return modifiers
}

function vue2AssignmentCodeForBridge(value, assignment) {
  const parsed = String(value || '').trim()
  const dot = parsed.lastIndexOf('.')
  if (dot > 0 && !parsed.slice(dot + 1).includes(']') && !parsed.slice(dot + 1).includes('[')) {
    return `$set(${parsed.slice(0, dot)}, ${JSON.stringify(parsed.slice(dot + 1))}, ${assignment})`
  }
  return `${parsed}=${assignment}`
}

function isVue2ComponentElementForBridge(node) {
  if (!node || typeof node !== 'object') return false
  if (node.component) return true
  const tag = String(node.tag || '')
  return !!tag && tag.includes('-')
}

function normalizeVue2ElementModelForBridge(node) {
  const model = node && node.model
  if (!model || typeof model !== 'object') return null
  const rawName = Array.isArray(node.directives)
    ? (node.directives.find(directive => directive && directive.name === 'model') || {}).rawName
    : undefined
  return {
    name: 'model',
    raw_name: String(rawName || 'v-model'),
    value: vue2ModelExpressionForBridge(model),
    arg: null,
    is_dynamic_arg: false,
    modifiers: {},
  }
}

function vue2ModelExpressionForBridge(model) {
  const expression = model && model.expression
  if (typeof expression === 'string') {
    try {
      return JSON.parse(expression)
    } catch (_) {
      return expression
    }
  }
  const value = model && model.value
  if (typeof value === 'string') return value.replace(/^\(([\s\S]*)\)$/, '$1')
  return ''
}

function normalizeVue2NodeForBridge(node) {
  if (!node || typeof node !== 'object') return node
  if ('Element' in node) return { Element: normalizeVue2AstForBridge(node.Element) }
  if ('Text' in node) return { Text: normalizeVue2TextForBridge(node.Text) }
  return normalizeVue2PublicNodeForBridge(node)
}

function normalizeVue2EventsForBridge(events) {
  if (!events || typeof events !== 'object') return {}
  const normalized = {}
  for (const key of Object.keys(events)) {
    const value = events[key]
    if (value === undefined) {
      normalized[key] = []
    } else if (Array.isArray(value)) {
      normalized[key] = value.map(normalizeVue2EventHandlerForBridge)
    } else {
      normalized[key] = [normalizeVue2EventHandlerForBridge(value)]
    }
  }
  return normalized
}

function normalizeVue2InternalEventsForBridge(events) {
  if (!events || typeof events !== 'object') return
  for (const key of Object.keys(events)) {
    if (events[key] === undefined) events[key] = []
  }
}

function normalizeVue2PublicNodeForBridge(node) {
  if (!node || typeof node !== 'object') return node
  if ('Element' in node) return { Element: normalizeVue2AstForBridge(node.Element) }
  if ('Text' in node) return { Text: normalizeVue2TextForBridge(node.Text) }
  if (node.type === 1 || typeof node.tag === 'string') {
    return { Element: normalizeVue2AstForBridge(node) }
  }
  return { Text: normalizeVue2TextForBridge(node) }
}

function normalizeVue2TextForBridge(node) {
  const expression = node && Object.prototype.hasOwnProperty.call(node, 'expression')
    ? node.expression
    : null
  return {
    text: String((node && node.text) || ''),
    expression,
    is_comment: Boolean(node && (node.isComment || node.is_comment)),
    static_node: Boolean(node && (node.static ?? node.static_node)),
  }
}

function normalizeVue2RawAttrsForBridge(attrs) {
  if (!Array.isArray(attrs)) return []
  return attrs.map(attr => ({
    name: String((attr && attr.name) || ''),
    value: String((attr && attr.value) || ''),
    dynamic: Boolean(attr && attr.dynamic),
  }))
}

function normalizeVue2AttrsForBridge(attrs) {
  if (!Array.isArray(attrs)) return []
  return attrs.map(attr => ({
    name: String((attr && attr.name) || ''),
    value: String((attr && attr.value) || ''),
    dynamic: Boolean(attr && attr.dynamic),
  }))
}

function normalizeVue2AttrsMapForBridge(attrsMap, attrsList) {
  if (attrsMap && typeof attrsMap === 'object') return { ...attrsMap }
  return Object.fromEntries(normalizeVue2RawAttrsForBridge(attrsList).map(attr => [attr.name, attr.value]))
}

function normalizeVue2RawAttrsMapForBridge(rawAttrsMap, attrsList) {
  if (rawAttrsMap && typeof rawAttrsMap === 'object') {
    return Object.fromEntries(
      Object.entries(rawAttrsMap).map(([key, attr]) => [key, {
        name: String((attr && attr.name) || key),
        value: String((attr && attr.value) || ''),
        dynamic: Boolean(attr && attr.dynamic),
      }])
    )
  }
  return Object.fromEntries(normalizeVue2RawAttrsForBridge(attrsList).map(attr => [attr.name, attr]))
}

function normalizeVue2DirectivesForBridge(directives) {
  if (!Array.isArray(directives)) return []
  return directives.map(directive => ({
    name: String((directive && directive.name) || ''),
    raw_name: String((directive && (directive.rawName || directive.raw_name)) || ''),
    value: directive && directive.value,
    arg: directive && directive.arg,
    is_dynamic_arg: Boolean(directive && (directive.isDynamicArg || directive.is_dynamic_arg)),
    modifiers: directive && directive.modifiers && typeof directive.modifiers === 'object' ? { ...directive.modifiers } : {},
  }))
}

function normalizeVue2EventHandlerForBridge(handler) {
  if (!handler || typeof handler !== 'object') {
    return {
      value: handler == null ? '' : String(handler),
      modifiers: {},
      modifier_order: [],
      has_modifier_object: false,
      dynamic: false,
    }
  }
  const modifiers = handler.modifiers && typeof handler.modifiers === 'object' ? { ...handler.modifiers } : {}
  return {
    value: String(handler.value || ''),
    modifiers,
    modifier_order: Array.isArray(handler.modifierOrder || handler.modifier_order)
      ? (handler.modifierOrder || handler.modifier_order).map(String)
      : Object.keys(modifiers),
    has_modifier_object: Boolean(handler.hasModifierObject || handler.has_modifier_object || Object.keys(modifiers).length > 0),
    dynamic: Boolean(handler.dynamic),
  }
}

function normalizeVue2ScopedSlotsForBridge(scopedSlots) {
  if (!scopedSlots || typeof scopedSlots !== 'object') return {}
  return Object.fromEntries(
    Object.entries(scopedSlots).map(([key, slot]) => {
      const normalized = normalizeVue2AstForBridge(slot)
      return [normalized.slot_target || quoteVue2SlotKeyForBridge(key, normalized.slot_target_dynamic), normalized]
    })
  )
}

function quoteVue2SlotKeyForBridge(key, dynamic) {
  if (dynamic || key.startsWith('"') || key.startsWith("'")) return key
  return JSON.stringify(key)
}

function emitVue2InlineTemplateWarnings(node) {
  if (!node || typeof node !== 'object') return
  if ((node.inline_template || node.inlineTemplate) && (!Array.isArray(node.children) || node.children.length !== 1)) {
    console.error('Inline-template components must have exactly one child element.')
  }
  if (Array.isArray(node.children)) {
    for (const child of node.children) {
      emitVue2InlineTemplateWarnings(child && child.Element ? child.Element : child)
    }
  }
  const scopedSlots = node.scoped_slots || node.scopedSlots
  if (scopedSlots && typeof scopedSlots === 'object') {
    for (const slot of Object.values(scopedSlots)) emitVue2InlineTemplateWarnings(slot)
  }
  const ifConditions = node.if_conditions || node.ifConditions
  if (Array.isArray(ifConditions)) {
    for (const condition of ifConditions.slice(1)) {
      emitVue2InlineTemplateWarnings(condition && condition.block)
    }
  }
}
"#,
    )?;
    write_text(
        &compiler_root.join("codeframe.ts"),
        r#"
import { generateCodeFrame } from 'vue-template-compiler'
export { generateCodeFrame }
"#,
    )?;
    write_text(
        &compiler_root.join("helpers.ts"),
        r#"
export function getAndRemoveAttr(el, name) {
  if (!el || !el.attrsMap || !(name in el.attrsMap)) return undefined
  const value = el.attrsMap[name]
  delete el.attrsMap[name]
  if (Array.isArray(el.attrsList)) {
    const index = el.attrsList.findIndex(attr => attr && attr.name === name)
    if (index >= 0) el.attrsList.splice(index, 1)
  }
  return value
}
"#,
    )?;

    let web_compiler = prepared_root
        .join("src")
        .join("platforms")
        .join("web")
        .join("compiler");
    fs::create_dir_all(&web_compiler)
        .with_context(|| format!("failed to create {}", web_compiler.display()))?;
    write_text(
        &web_compiler.join("index.ts"),
        r#"
import * as vueTemplateCompiler from 'vue-template-compiler'
import { parse } from 'compiler/parser'
import { optimize } from 'compiler/optimizer'
import { generate } from 'compiler/codegen'

export function compile(template, options = {}) {
  if (!usesVue2JavascriptCompilerCallbacks(options)) {
    return vueTemplateCompiler.compile(template, options)
  }
  const ast = parse(template, Object.assign({}, options, { optimize: false }))
  if (!ast) return vueTemplateCompiler.compile(template, options)
  runVue2CompileModuleTransforms(ast, options, 'transformNode')
  runVue2DirectiveTransforms(ast, options)
  optimize(ast, options)
  const generated = generate(ast, options)
  return {
    ast,
    render: generated.render,
    staticRenderFns: generated.staticRenderFns || [],
    errors: [],
    tips: [],
  }
}

function usesVue2JavascriptCompilerCallbacks(options) {
  if (!options || typeof options !== 'object') return false
  if (Array.isArray(options.modules)) {
    for (const module of options.modules) {
      if (!module || typeof module !== 'object') continue
      if (
        typeof module.transformNode === 'function' ||
        typeof module.preTransformNode === 'function' ||
        typeof module.postTransformNode === 'function' ||
        typeof module.genData === 'function' ||
        typeof module.transformCode === 'function'
      ) {
        return true
      }
    }
  }
  if (options.directives && typeof options.directives === 'object') {
    for (const directive of Object.values(options.directives)) {
      if (typeof directive === 'function') return true
    }
  }
  return false
}

function runVue2CompileModuleTransforms(ast, options, hook) {
  if (!ast || !options || !Array.isArray(options.modules)) return
  walkVue2CompileElements(ast, element => {
    for (const module of options.modules) {
      const transform = module && module[hook]
      if (typeof transform === 'function') transform(element, options)
    }
    syncVue2RawAttrsFromPublicAttrs(element)
  })
}

function runVue2DirectiveTransforms(ast, options) {
  if (!ast || !options || !options.directives || typeof options.directives !== 'object') return
  walkVue2CompileElements(ast, element => {
    if (!Array.isArray(element.directives)) return
    element.directives = element.directives.filter(directive => {
      const transform = options.directives[directive && directive.name]
      if (typeof transform !== 'function') return true
      return !!transform(element, directive, options)
    })
  })
}

function walkVue2CompileElements(node, visit) {
  if (!node || typeof node !== 'object') return
  if ('Element' in node) return walkVue2CompileElements(node.Element, visit)
  if (typeof node.tag !== 'string') return
  visit(node)
  if (Array.isArray(node.children)) {
    for (const child of node.children) walkVue2CompileElements(child && (child.Element || child), visit)
  }
  const conditions = node.ifConditions || node.if_conditions
  if (Array.isArray(conditions)) {
    for (const condition of conditions) walkVue2CompileElements(condition && condition.block, visit)
  }
  const scopedSlots = node.scopedSlots || node.scoped_slots
  if (scopedSlots && typeof scopedSlots === 'object') {
    for (const slot of Object.values(scopedSlots)) walkVue2CompileElements(slot, visit)
  }
}

function syncVue2RawAttrsFromPublicAttrs(element) {
  if (!element || typeof element !== 'object' || !Array.isArray(element.attrsList)) return
  const names = new Set(element.attrsList.map(attr => attr && attr.name).filter(Boolean))
  if (element.attrsMap && typeof element.attrsMap === 'object') {
    for (const name of Object.keys(element.attrsMap)) {
      if (!names.has(name)) delete element.attrsMap[name]
    }
  }
  if (element.rawAttrsMap && typeof element.rawAttrsMap === 'object') {
    for (const name of Object.keys(element.rawAttrsMap)) {
      if (!names.has(name)) delete element.rawAttrsMap[name]
    }
  }
  if (Array.isArray(element.attrs)) {
    element.attrs = element.attrs.filter(attr => attr && names.has(attr.name))
  }
}
"#,
    )?;
    write_text(
        &web_compiler.join("options.ts"),
        r#"
export const baseOptions = {
  expectHTML: true,
  modules: [],
  directives: {},
  isPreTag: tag => tag === 'pre',
  isUnaryTag: tag => /^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(tag),
  mustUseProp: () => false,
  canBeLeftOpenTag: () => false,
  isReservedTag: tag => /^(html|body|base|head|link|meta|style|title|address|article|aside|footer|header|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|rtc|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot)$/i.test(tag),
  getTagNamespace: tag => tag === 'svg' ? 'svg' : undefined,
  staticKeys: '',
}
"#,
    )?;

    let web_util = prepared_root
        .join("src")
        .join("platforms")
        .join("web")
        .join("util");
    fs::create_dir_all(&web_util)
        .with_context(|| format!("failed to create {}", web_util.display()))?;
    write_text(
        &web_util.join("index.ts"),
        r#"
export const isReservedTag = tag => /^(html|body|base|head|link|meta|style|title|address|article|aside|footer|header|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|rtc|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot)$/i.test(tag)
"#,
    )?;

    let shared = prepared_root.join("src").join("shared");
    fs::create_dir_all(&shared)
        .with_context(|| format!("failed to create {}", shared.display()))?;
    write_text(
        &shared.join("util.ts"),
        r#"
export const isObject = value => value !== null && typeof value === 'object'
export const isFunction = value => typeof value === 'function'
export function extend(to, from) {
  return Object.assign(to, from)
}
export const noop = () => {}
"#,
    )?;

    let core_util = prepared_root.join("src").join("core").join("util");
    fs::create_dir_all(&core_util)
        .with_context(|| format!("failed to create {}", core_util.display()))?;
    write_text(
        &core_util.join("env.ts"),
        r#"
export const isIE = false
export const isEdge = false
"#,
    )?;

    let web_entry = prepared_root.join("src").join("platforms").join("web");
    write_text(
        &web_entry.join("entry-compiler.ts"),
        r#"
import Vue from 'vue'
export default Vue
export * from './compiler'
"#,
    )?;

    if include_types {
        let types_root = prepared_root.join("src").join("types");
        fs::create_dir_all(&types_root)
            .with_context(|| format!("failed to create {}", types_root.display()))?;
        write_text(
            &types_root.join("compiler.ts"),
            "export const WarningMessage = String\n",
        )?;
        let sfc_src = prepared_root
            .join("packages")
            .join("compiler-sfc")
            .join("src");
        fs::create_dir_all(&sfc_src)
            .with_context(|| format!("failed to create {}", sfc_src.display()))?;
        write_text(
            &sfc_src.join("types.ts"),
            r#"
export const BindingTypes = {
  DATA: 'data',
  PROPS: 'props',
  PROPS_ALIASED: 'props-aliased',
  SETUP_LET: 'setup-let',
  SETUP_CONST: 'setup-const',
  SETUP_REACTIVE_CONST: 'setup-reactive-const',
  SETUP_MAYBE_REF: 'setup-maybe-ref',
  SETUP_REF: 'setup-ref',
  OPTIONS: 'options',
  LITERAL_CONST: 'literal-const',
}
"#,
        )?;
    }

    Ok(())
}
