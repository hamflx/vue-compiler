use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3DomMirRenderMode {
    Root,
    Child,
}

pub(crate) fn push_unique_helper(helpers: &mut Vec<RuntimeHelper>, helper: RuntimeHelper) {
    if !helpers.contains(&helper) {
        helpers.push(helper);
    }
}

pub(crate) fn push_vue3_dom_binding_helpers(
    binding: &Vue3DomBinding,
    helpers: &mut Vec<RuntimeHelper>,
) {
    if !binding.dynamic_arg && binding.name == "class" {
        push_unique_helper(helpers, RuntimeHelper::Vue3NormalizeClass);
    }
    if binding.dynamic_arg && binding.camel {
        push_unique_helper(helpers, RuntimeHelper::Vue3Camelize);
    }
}

pub(crate) fn push_vue3_dom_event_helpers(event: &Vue3DomEvent, helpers: &mut Vec<RuntimeHelper>) {
    if event.dynamic_arg {
        push_unique_helper(helpers, RuntimeHelper::Vue3ToHandlerKey);
    }
    if !event.runtime_modifiers.is_empty() {
        push_unique_helper(helpers, RuntimeHelper::Vue3WithModifiers);
    }
    if !event.key_modifiers.is_empty() {
        push_unique_helper(helpers, RuntimeHelper::Vue3WithKeys);
    }
}

pub(crate) fn render_dynamic_prop_key(key: &str) -> String {
    format!("{} || \"\"", key.trim())
}

pub(crate) fn render_vue3_dom_binding_static_key(
    binding: &Vue3DomBinding,
    apply_dom_prefix: bool,
) -> String {
    let mut name = if binding.camel {
        camelize(&binding.name)
    } else {
        binding.name.clone()
    };
    if apply_dom_prefix {
        if binding.force_prop {
            name = format!(".{name}");
        } else if binding.force_attr {
            name = format!("^{name}");
        }
    }
    name
}

pub(crate) fn render_vue3_dom_binding_dynamic_key(
    binding: &Vue3DomBinding,
    name: String,
    apply_dom_prefix: bool,
) -> String {
    if !apply_dom_prefix && !binding.camel {
        return name.trim().to_string();
    }
    let mut key = render_dynamic_prop_key(&name);
    if binding.camel {
        key = format!("_camelize({key})");
    }
    if apply_dom_prefix {
        if binding.force_prop {
            key = format!("'.' + ({key})");
        } else if binding.force_attr {
            key = format!("'^' + ({key})");
        }
    }
    key
}

pub(crate) fn props_requires_merge_call(props: &Vue3DomProps) -> bool {
    let mut args = 0usize;
    let mut pending_object_entries = false;
    for segment in &props.segments {
        match segment {
            Vue3DomPropSegment::StaticAttr(_)
            | Vue3DomPropSegment::DynamicBinding(_)
            | Vue3DomPropSegment::Content(_)
            | Vue3DomPropSegment::Model(_)
            | Vue3DomPropSegment::Event(_) => pending_object_entries = true,
            Vue3DomPropSegment::ObjectBinding(_) | Vue3DomPropSegment::ObjectListeners(_) => {
                if pending_object_entries {
                    args += 1;
                    pending_object_entries = false;
                }
                args += 1;
            }
        }
    }
    if pending_object_entries {
        args += 1;
    }
    args > 1
}

pub(crate) fn vue3_dom_content_text_is_static(
    content: &Vue3DomContent,
    js: &JsAstStore,
    options: &Vue3CompilerOptions,
) -> bool {
    let Vue3DomContent::Text {
        expression: Some(expression),
    } = content
    else {
        return true;
    };
    let Some(entry) = js.expressions().get(expression.0 as usize) else {
        return false;
    };
    let source = entry.source.trim();
    process_expression_is_static_literal(source)
        || matches!(
            options.binding_metadata.get(source).map(String::as_str),
            Some("literal-const")
        )
        || vue3_expression_is_string_literal(source)
}

pub(crate) fn vue3_expression_is_string_literal(source: &str) -> bool {
    let mut chars = source.chars();
    let Some(quote) = chars.next() else {
        return false;
    };
    if !matches!(quote, '\'' | '"') || !source.ends_with(quote) || source.len() < 2 {
        return false;
    }
    let mut escaped = false;
    for ch in source[quote.len_utf8()..source.len() - quote.len_utf8()].chars() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return false;
        }
    }
    !escaped
}

pub(crate) fn ssr_attrs_has_object_binding(props: &Vue3DomProps) -> bool {
    if props.segments.is_empty() {
        !props.object_bindings.is_empty()
    } else {
        props
            .segments
            .iter()
            .any(|segment| matches!(segment, Vue3DomPropSegment::ObjectBinding(_)))
    }
}

pub(crate) fn ssr_attrs_prop_chunk_count(props: &Vue3DomProps) -> usize {
    let mut chunks = 0usize;
    let mut pending_object = false;
    if props.segments.is_empty() {
        if !props.static_attrs.is_empty() || !props.dynamic_bindings.is_empty() {
            chunks += 1;
        }
        chunks += props.object_bindings.len();
        return chunks;
    }
    for segment in &props.segments {
        match segment {
            Vue3DomPropSegment::StaticAttr(_) | Vue3DomPropSegment::DynamicBinding(_) => {
                pending_object = true;
            }
            Vue3DomPropSegment::ObjectBinding(_) => {
                if pending_object {
                    chunks += 1;
                    pending_object = false;
                }
                chunks += 1;
            }
            Vue3DomPropSegment::Content(_)
            | Vue3DomPropSegment::Model(_)
            | Vue3DomPropSegment::Event(_)
            | Vue3DomPropSegment::ObjectListeners(_) => {}
        }
    }
    if pending_object {
        chunks += 1;
    }
    chunks
}

pub(crate) fn vue3_ssr_is_boolean_attr(name: &str) -> bool {
    matches!(
        name,
        "allowfullscreen"
            | "async"
            | "autofocus"
            | "autoplay"
            | "checked"
            | "controls"
            | "default"
            | "defer"
            | "disabled"
            | "formnovalidate"
            | "hidden"
            | "inert"
            | "ismap"
            | "itemscope"
            | "loop"
            | "multiple"
            | "muted"
            | "nomodule"
            | "novalidate"
            | "open"
            | "readonly"
            | "required"
            | "reversed"
            | "scoped"
            | "seamless"
            | "selected"
    )
}

pub(crate) fn vue3_slot_flag_value(flag: Vue3SlotFlag) -> u8 {
    match flag {
        Vue3SlotFlag::Stable => 1,
        Vue3SlotFlag::Dynamic => 2,
        Vue3SlotFlag::Forwarded => 3,
    }
}

pub(crate) fn vue3_slot_flag_with_comment(flag: Vue3SlotFlag) -> String {
    match flag {
        Vue3SlotFlag::Stable => "1 /* STABLE */".into(),
        Vue3SlotFlag::Dynamic => "2 /* DYNAMIC */".into(),
        Vue3SlotFlag::Forwarded => "3 /* FORWARDED */".into(),
    }
}

pub(crate) fn dom_tag_consumes_binding(tag: &Vue3DomTag, binding: &Vue3DomBinding) -> bool {
    matches!(tag, Vue3DomTag::DynamicComponent(_)) && !binding.dynamic_arg && binding.name == "is"
}

pub(crate) fn vue3_dom_model_runtime_helper(kind: Vue3DomModelKind) -> RuntimeHelper {
    match kind {
        Vue3DomModelKind::Text => RuntimeHelper::Vue3VModelText,
        Vue3DomModelKind::Radio => RuntimeHelper::Vue3VModelRadio,
        Vue3DomModelKind::Checkbox => RuntimeHelper::Vue3VModelCheckbox,
        Vue3DomModelKind::Select => RuntimeHelper::Vue3VModelSelect,
        Vue3DomModelKind::Dynamic => RuntimeHelper::Vue3VModelDynamic,
    }
}

pub(crate) fn vue3_codegen_root(ast: &Vue3Ast) -> Option<&Vue3Root> {
    ast.root_node().and_then(|node| match &node.kind {
        Vue3AstKind::Root(root) => Some(root),
        _ => None,
    })
}

pub(crate) fn vue3_root_has_codegen_state(root: &Vue3Root) -> bool {
    !root.helpers.is_empty() || !root.components.is_empty() || !root.directives.is_empty()
}

pub(crate) fn vue3_codegen_imports(ast: &Vue3Ast) -> Vec<String> {
    vue3_codegen_root(ast)
        .into_iter()
        .flat_map(|root| root.imports.iter())
        .map(|import| format!("import {} from '{}'", import.name, import.path))
        .collect()
}

pub(crate) fn vue3_codegen_components(ast: &Vue3Ast) -> Vec<String> {
    if let Some(root) = vue3_codegen_root(ast).filter(|root| vue3_root_has_codegen_state(root)) {
        root.components.iter().cloned().collect()
    } else {
        collect_component_tags(ast)
    }
}

pub(crate) fn render_component_declarations(components: &[String]) -> Vec<String> {
    components
        .iter()
        .map(|component| {
            format!(
                "const {} = _resolveComponent({})",
                component_asset_id(component),
                quote_string(component)
            )
        })
        .collect()
}

pub(crate) fn vue3_codegen_directives(ast: &Vue3Ast) -> Vec<String> {
    if let Some(root) = vue3_codegen_root(ast).filter(|root| vue3_root_has_codegen_state(root)) {
        root.directives.iter().cloned().collect()
    } else {
        collect_runtime_directive_names(ast)
    }
}

pub(crate) fn render_directive_declarations(directives: &[String]) -> Vec<String> {
    directives
        .iter()
        .map(|directive| {
            format!(
                "const {} = _resolveDirective({})",
                directive_asset_id(directive),
                quote_string(directive)
            )
        })
        .collect()
}

pub(crate) fn vue3_codegen_helpers(
    ast: &Vue3Ast,
    ctx: &TransformContext,
    declarations: &[String],
    expr: &str,
    has_components: bool,
    preserve_stringify_static_helpers: bool,
) -> Vec<RuntimeHelper> {
    let helper_probe = format!("{}\n{}", declarations.join("\n"), expr);
    let mut stringify_static_source_helpers = Vec::new();
    let mut helpers =
        if let Some(root) = vue3_codegen_root(ast).filter(|root| !root.helpers.is_empty()) {
            let helpers = root.helpers.iter().copied().collect::<Vec<_>>();
            stringify_static_source_helpers = helpers.clone();
            prune_helpers_to_rendered_code(helpers, &helper_probe, has_components)
        } else if !ctx.helpers.is_empty() {
            let helpers = ctx.helpers.iter().copied().collect::<Vec<_>>();
            stringify_static_source_helpers = helpers.clone();
            prune_helpers_to_rendered_code(helpers, &helper_probe, has_components)
        } else {
            let mut helpers =
                render_helpers_from_code(vue3_helper_order(has_components), &helper_probe);
            let needs_comment_helper = helper_probe.contains("_createCommentVNode(")
                || helper_probe.contains("? (_openBlock()")
                || helper_probe.contains("? _withMemo(");
            if needs_comment_helper && !helpers.contains(&RuntimeHelper::Vue3CreateCommentVNode) {
                helpers.push(RuntimeHelper::Vue3CreateCommentVNode);
            }
            helpers
        };
    if has_components && !helpers.contains(&RuntimeHelper::Vue3ResolveComponent) {
        helpers.push(RuntimeHelper::Vue3ResolveComponent);
    }
    helpers.dedup();
    sort_helpers_by_order(&mut helpers, vue3_helper_order(has_components));
    apply_vue3_stringify_static_helper_order(
        &mut helpers,
        &helper_probe,
        &stringify_static_source_helpers,
        preserve_stringify_static_helpers,
    );
    apply_vue3_memo_helper_order(&mut helpers);
    apply_vue3_plain_child_helper_order(&mut helpers);
    apply_vue3_cached_children_helper_order(&mut helpers, &helper_probe);
    apply_vue3_transition_helper_order(&mut helpers, &helper_probe);
    apply_vue3_dynamic_event_helper_order(&mut helpers, &helper_probe);
    helpers
}

pub(crate) fn prune_helpers_to_rendered_code(
    helpers: Vec<RuntimeHelper>,
    helper_probe: &str,
    has_components: bool,
) -> Vec<RuntimeHelper> {
    let mut pruned = render_helpers_from_code(vue3_helper_order(has_components), helper_probe);
    let keep_transition_comment_helper = helpers.contains(&RuntimeHelper::Vue3CreateCommentVNode)
        && helpers.contains(&RuntimeHelper::Vue3Transition)
        && helper_probe.contains("_Transition");
    let needs_comment_helper = helper_probe.contains("_createCommentVNode(")
        || helper_probe.contains("? (_openBlock()")
        || helper_probe.contains("? _withMemo(")
        || keep_transition_comment_helper;
    for helper in helpers {
        if pruned.contains(&helper) {
            continue;
        }
        match helper {
            RuntimeHelper::Vue3CreateCommentVNode if needs_comment_helper => {
                pruned.push(helper);
            }
            RuntimeHelper::Vue3WithMemo
                if helper_probe.contains(&helper_reference(RuntimeHelper::Vue3IsMemoSame)) =>
            {
                pruned.push(helper);
            }
            _ => {}
        }
    }
    pruned
}

pub(crate) fn apply_vue3_memo_helper_order(helpers: &mut Vec<RuntimeHelper>) {
    if !helpers.contains(&RuntimeHelper::Vue3WithMemo) {
        return;
    }
    if helpers.contains(&RuntimeHelper::Vue3IsMemoSame) {
        move_helper_to_start(helpers, RuntimeHelper::Vue3RenderList);
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
        );
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3IsMemoSame,
            RuntimeHelper::Vue3CreateElementVNode,
        );
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3IsMemoSame,
        );
    } else if helpers.contains(&RuntimeHelper::Vue3ResolveComponent) {
        if helpers.contains(&RuntimeHelper::Vue3CreateVNode) {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3ResolveComponent,
                RuntimeHelper::Vue3OpenBlock,
            );
            move_helper_after(
                helpers,
                RuntimeHelper::Vue3CreateVNode,
                RuntimeHelper::Vue3ResolveComponent,
            );
            move_helper_after(
                helpers,
                RuntimeHelper::Vue3WithMemo,
                RuntimeHelper::Vue3CreateVNode,
            );
        } else {
            reorder_helpers_by_preference(
                helpers,
                &[
                    RuntimeHelper::Vue3CreateElementVNode,
                    RuntimeHelper::Vue3CreateTextVNode,
                    RuntimeHelper::Vue3OpenBlock,
                    RuntimeHelper::Vue3CreateElementBlock,
                    RuntimeHelper::Vue3WithMemo,
                    RuntimeHelper::Vue3CreateCommentVNode,
                    RuntimeHelper::Vue3ResolveComponent,
                    RuntimeHelper::Vue3CreateBlock,
                ],
            );
        }
    } else {
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3CreateElementBlock,
        );
    }
}

pub(crate) fn apply_vue3_stringify_static_helper_order(
    helpers: &mut Vec<RuntimeHelper>,
    helper_probe: &str,
    source_helpers: &[RuntimeHelper],
    preserve_source_helpers: bool,
) {
    if !helpers.contains(&RuntimeHelper::Vue3CreateStaticVNode)
        || !helper_probe.contains("_createStaticVNode(")
    {
        return;
    }
    if !helpers.contains(&RuntimeHelper::Vue3CreateElementVNode) {
        helpers.push(RuntimeHelper::Vue3CreateElementVNode);
    }
    let preference = [
        RuntimeHelper::Vue3SetBlockTracking,
        RuntimeHelper::Vue3ToDisplayString,
        RuntimeHelper::Vue3NormalizeClass,
        RuntimeHelper::Vue3CreateCommentVNode,
        RuntimeHelper::Vue3CreateTextVNode,
        RuntimeHelper::Vue3CreateElementVNode,
        RuntimeHelper::Vue3CreateStaticVNode,
        RuntimeHelper::Vue3Fragment,
        RuntimeHelper::Vue3OpenBlock,
        RuntimeHelper::Vue3CreateElementBlock,
    ];
    let keep_source_helpers = [
        RuntimeHelper::Vue3SetBlockTracking,
        RuntimeHelper::Vue3ToDisplayString,
        RuntimeHelper::Vue3CreateCommentVNode,
    ];
    for helper in keep_source_helpers {
        if source_helpers.contains(&helper) && !helpers.contains(&helper) {
            helpers.push(helper);
        }
    }
    if preserve_source_helpers
        && source_helpers.contains(&RuntimeHelper::Vue3NormalizeClass)
        && !helpers.contains(&RuntimeHelper::Vue3NormalizeClass)
    {
        helpers.push(RuntimeHelper::Vue3NormalizeClass);
    }
    reorder_helpers_by_preference(helpers, &preference);
}

pub(crate) fn apply_vue3_plain_child_helper_order(helpers: &mut Vec<RuntimeHelper>) {
    let fragment_child_order = [
        RuntimeHelper::Vue3CreateElementVNode,
        RuntimeHelper::Vue3Fragment,
        RuntimeHelper::Vue3OpenBlock,
        RuntimeHelper::Vue3CreateElementBlock,
    ];
    if helpers.len() == fragment_child_order.len()
        && fragment_child_order
            .iter()
            .all(|helper| helpers.contains(helper))
    {
        helpers.clear();
        helpers.extend(fragment_child_order);
        return;
    }

    let fragment_content_prop_order = [
        RuntimeHelper::Vue3ToDisplayString,
        RuntimeHelper::Vue3CreateElementVNode,
        RuntimeHelper::Vue3NormalizeStyle,
        RuntimeHelper::Vue3Fragment,
        RuntimeHelper::Vue3OpenBlock,
        RuntimeHelper::Vue3CreateElementBlock,
    ];
    if helpers.len() == fragment_content_prop_order.len()
        && fragment_content_prop_order
            .iter()
            .all(|helper| helpers.contains(helper))
    {
        helpers.clear();
        helpers.extend(fragment_content_prop_order);
        return;
    }

    let plain_child_order = [
        RuntimeHelper::Vue3ToDisplayString,
        RuntimeHelper::Vue3CreateElementVNode,
        RuntimeHelper::Vue3OpenBlock,
        RuntimeHelper::Vue3CreateElementBlock,
    ];
    if helpers.len() == plain_child_order.len()
        && plain_child_order
            .iter()
            .all(|helper| helpers.contains(helper))
    {
        helpers.clear();
        helpers.extend(plain_child_order);
    }
}

pub(crate) fn apply_vue3_cached_children_helper_order(
    helpers: &mut Vec<RuntimeHelper>,
    helper_probe: &str,
) {
    if helpers.contains(&RuntimeHelper::Vue3CreateStaticVNode)
        || !helper_probe.contains("_cache[")
        || !helper_probe.contains("/* CACHED */")
    {
        return;
    }
    reorder_helpers_by_preference(
        helpers,
        &[
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
        ],
    );
}

pub(crate) fn apply_vue3_transition_helper_order(
    helpers: &mut Vec<RuntimeHelper>,
    helper_probe: &str,
) {
    if !helpers.contains(&RuntimeHelper::Vue3Transition) {
        return;
    }
    if helpers.contains(&RuntimeHelper::Vue3CreateCommentVNode)
        && helpers.contains(&RuntimeHelper::Vue3CreateElementVNode)
        && !helper_probe.contains("_Fragment")
    {
        reorder_helpers_by_preference(
            helpers,
            &[
                RuntimeHelper::Vue3CreateCommentVNode,
                RuntimeHelper::Vue3CreateElementVNode,
                RuntimeHelper::Vue3Transition,
                RuntimeHelper::Vue3WithCtx,
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateBlock,
            ],
        );
        return;
    }
    if helpers.contains(&RuntimeHelper::Vue3VShow) {
        reorder_helpers_by_preference(
            helpers,
            &[
                RuntimeHelper::Vue3VShow,
                RuntimeHelper::Vue3CreateElementVNode,
                RuntimeHelper::Vue3WithDirectives,
                RuntimeHelper::Vue3Transition,
                RuntimeHelper::Vue3WithCtx,
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateBlock,
            ],
        );
        return;
    }
    if helper_probe.contains("_Fragment") && helper_probe.contains("_createCommentVNode(") {
        reorder_helpers_by_preference(
            helpers,
            &[
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateElementBlock,
                RuntimeHelper::Vue3CreateCommentVNode,
                RuntimeHelper::Vue3CreateElementVNode,
                RuntimeHelper::Vue3Fragment,
                RuntimeHelper::Vue3Transition,
                RuntimeHelper::Vue3WithCtx,
                RuntimeHelper::Vue3CreateBlock,
            ],
        );
    }
}

pub(crate) fn apply_vue3_dynamic_event_helper_order(
    helpers: &mut Vec<RuntimeHelper>,
    helper_probe: &str,
) {
    if !helpers.contains(&RuntimeHelper::Vue3ToHandlerKey) {
        return;
    }
    if !helper_probe.contains("[_toHandlerKey(") {
        return;
    }
    let preferred = [
        RuntimeHelper::Vue3ToHandlerKey,
        RuntimeHelper::Vue3MergeProps,
        RuntimeHelper::Vue3OpenBlock,
        RuntimeHelper::Vue3CreateElementBlock,
    ];
    reorder_helpers_by_preference(helpers, &preferred);
}

pub(crate) fn sort_helpers_by_order(helpers: &mut [RuntimeHelper], order: &[RuntimeHelper]) {
    helpers.sort_by_key(|helper| {
        order
            .iter()
            .position(|candidate| candidate == helper)
            .unwrap_or(order.len())
    });
}

pub(crate) fn reorder_helpers_by_preference(
    helpers: &mut Vec<RuntimeHelper>,
    preferred: &[RuntimeHelper],
) {
    helpers.dedup();
    let mut reordered = Vec::with_capacity(helpers.len());
    for helper in preferred {
        if helpers.contains(helper) {
            reordered.push(*helper);
        }
    }
    for helper in helpers.iter().copied() {
        if !reordered.contains(&helper) {
            reordered.push(helper);
        }
    }
    *helpers = reordered;
}

pub(crate) fn move_helper_to_start(helpers: &mut Vec<RuntimeHelper>, helper: RuntimeHelper) {
    let Some(index) = helpers.iter().position(|candidate| *candidate == helper) else {
        return;
    };
    let helper = helpers.remove(index);
    helpers.insert(0, helper);
}

pub(crate) fn move_helper_after(
    helpers: &mut Vec<RuntimeHelper>,
    helper: RuntimeHelper,
    after: RuntimeHelper,
) {
    let Some(index) = helpers.iter().position(|candidate| *candidate == helper) else {
        return;
    };
    let helper = helpers.remove(index);
    if let Some(after_index) = helpers.iter().position(|candidate| *candidate == after) {
        helpers.insert(after_index + 1, helper);
    } else {
        helpers.push(helper);
    }
}

pub(crate) fn move_helper_before(
    helpers: &mut Vec<RuntimeHelper>,
    helper: RuntimeHelper,
    before: RuntimeHelper,
) {
    let Some(index) = helpers.iter().position(|candidate| *candidate == helper) else {
        return;
    };
    let helper = helpers.remove(index);
    if let Some(before_index) = helpers.iter().position(|candidate| *candidate == before) {
        helpers.insert(before_index, helper);
    } else {
        helpers.push(helper);
    }
}

pub(crate) fn move_helper_before_if_present(
    helpers: &mut Vec<RuntimeHelper>,
    helper: RuntimeHelper,
    before: RuntimeHelper,
) {
    if helpers.contains(&before) {
        move_helper_before(helpers, helper, before);
    }
}

pub(crate) fn vue3_helper_order(components_first: bool) -> &'static [RuntimeHelper] {
    if components_first {
        &[
            RuntimeHelper::Vue3SetBlockTracking,
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3CreateTextVNode,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3ResolveDynamicComponent,
            RuntimeHelper::Vue3BaseTransition,
            RuntimeHelper::Vue3Transition,
            RuntimeHelper::Vue3TransitionGroup,
            RuntimeHelper::Vue3Teleport,
            RuntimeHelper::Vue3Suspense,
            RuntimeHelper::Vue3KeepAlive,
            RuntimeHelper::Vue3WithCtx,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateSlots,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateBlock,
            RuntimeHelper::Vue3CreateVNode,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3CreateStaticVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3NormalizeProps,
            RuntimeHelper::Vue3NormalizeStyle,
            RuntimeHelper::Vue3GuardReactiveProps,
            RuntimeHelper::Vue3MergeProps,
            RuntimeHelper::Vue3ResolveDirective,
            RuntimeHelper::Vue3WithDirectives,
            RuntimeHelper::Vue3IsMemoSame,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3ToHandlers,
            RuntimeHelper::Vue3Camelize,
            RuntimeHelper::Vue3Capitalize,
            RuntimeHelper::Vue3ToHandlerKey,
            RuntimeHelper::Vue3PushScopeId,
            RuntimeHelper::Vue3PopScopeId,
            RuntimeHelper::Vue3Unref,
            RuntimeHelper::Vue3IsRef,
            RuntimeHelper::Vue3VModelRadio,
            RuntimeHelper::Vue3VModelCheckbox,
            RuntimeHelper::Vue3VModelText,
            RuntimeHelper::Vue3VModelSelect,
            RuntimeHelper::Vue3VModelDynamic,
            RuntimeHelper::Vue3VShow,
        ]
    } else {
        &[
            RuntimeHelper::Vue3SetBlockTracking,
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3CreateTextVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3CreateStaticVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3ResolveDynamicComponent,
            RuntimeHelper::Vue3BaseTransition,
            RuntimeHelper::Vue3Transition,
            RuntimeHelper::Vue3TransitionGroup,
            RuntimeHelper::Vue3Teleport,
            RuntimeHelper::Vue3Suspense,
            RuntimeHelper::Vue3KeepAlive,
            RuntimeHelper::Vue3WithCtx,
            RuntimeHelper::Vue3CreateBlock,
            RuntimeHelper::Vue3CreateVNode,
            RuntimeHelper::Vue3CreateSlots,
            RuntimeHelper::Vue3ResolveDirective,
            RuntimeHelper::Vue3WithDirectives,
            RuntimeHelper::Vue3IsMemoSame,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3NormalizeProps,
            RuntimeHelper::Vue3NormalizeStyle,
            RuntimeHelper::Vue3GuardReactiveProps,
            RuntimeHelper::Vue3MergeProps,
            RuntimeHelper::Vue3ToHandlers,
            RuntimeHelper::Vue3Camelize,
            RuntimeHelper::Vue3Capitalize,
            RuntimeHelper::Vue3ToHandlerKey,
            RuntimeHelper::Vue3PushScopeId,
            RuntimeHelper::Vue3PopScopeId,
            RuntimeHelper::Vue3Unref,
            RuntimeHelper::Vue3IsRef,
            RuntimeHelper::Vue3VModelRadio,
            RuntimeHelper::Vue3VModelCheckbox,
            RuntimeHelper::Vue3VModelText,
            RuntimeHelper::Vue3VModelSelect,
            RuntimeHelper::Vue3VModelDynamic,
            RuntimeHelper::Vue3VShow,
        ]
    }
}

pub(crate) fn helper_reference(helper: RuntimeHelper) -> String {
    format!("_{}", helper_name(helper))
}

pub(crate) fn helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{}: _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn import_helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{} as _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn render_args(options: &Vue3CompilerOptions) -> String {
    if options.is_ts {
        if options.binding_metadata.is_empty() || options.inline {
            return "_ctx: any,_cache: any".into();
        }
        return "_ctx: any,_cache: any,$props: any,$setup: any,$data: any,$options: any".into();
    }
    if options.binding_metadata.is_empty() || options.inline {
        "_ctx, _cache".into()
    } else {
        "_ctx, _cache, $props, $setup, $data, $options".into()
    }
}

pub(crate) fn helper_name(helper: RuntimeHelper) -> &'static str {
    match helper {
        RuntimeHelper::Vue2CreateElement => "createElement",
        RuntimeHelper::Vue2CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue2ToString => "toString",
        RuntimeHelper::Vue2RenderList => "renderList",
        RuntimeHelper::Vue2ResolveFilter => "resolveFilter",
        RuntimeHelper::Vue3ResolveDirective => "resolveDirective",
        RuntimeHelper::Vue3WithDirectives => "withDirectives",
        RuntimeHelper::Vue3SetBlockTracking => "setBlockTracking",
        RuntimeHelper::Vue3OpenBlock => "openBlock",
        RuntimeHelper::Vue3CreateElementVNode => "createElementVNode",
        RuntimeHelper::Vue3CreateElementBlock => "createElementBlock",
        RuntimeHelper::Vue3CreateCommentVNode => "createCommentVNode",
        RuntimeHelper::Vue3CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue3BaseTransition => "BaseTransition",
        RuntimeHelper::Vue3Transition => "Transition",
        RuntimeHelper::Vue3TransitionGroup => "TransitionGroup",
        RuntimeHelper::Vue3Teleport => "Teleport",
        RuntimeHelper::Vue3Suspense => "Suspense",
        RuntimeHelper::Vue3KeepAlive => "KeepAlive",
        RuntimeHelper::Vue3Fragment => "Fragment",
        RuntimeHelper::Vue3ToDisplayString => "toDisplayString",
        RuntimeHelper::Vue3RenderList => "renderList",
        RuntimeHelper::Vue3RenderSlot => "renderSlot",
        RuntimeHelper::Vue3NormalizeClass => "normalizeClass",
        RuntimeHelper::Vue3NormalizeProps => "normalizeProps",
        RuntimeHelper::Vue3NormalizeStyle => "normalizeStyle",
        RuntimeHelper::Vue3GuardReactiveProps => "guardReactiveProps",
        RuntimeHelper::Vue3MergeProps => "mergeProps",
        RuntimeHelper::Vue3ResolveComponent => "resolveComponent",
        RuntimeHelper::Vue3ResolveDynamicComponent => "resolveDynamicComponent",
        RuntimeHelper::Vue3WithCtx => "withCtx",
        RuntimeHelper::Vue3CreateBlock => "createBlock",
        RuntimeHelper::Vue3CreateVNode => "createVNode",
        RuntimeHelper::Vue3CreateSlots => "createSlots",
        RuntimeHelper::Vue3CreateStaticVNode => "createStaticVNode",
        RuntimeHelper::Vue3IsMemoSame => "isMemoSame",
        RuntimeHelper::Vue3WithMemo => "withMemo",
        RuntimeHelper::Vue3ToHandlers => "toHandlers",
        RuntimeHelper::Vue3Camelize => "camelize",
        RuntimeHelper::Vue3Capitalize => "capitalize",
        RuntimeHelper::Vue3ToHandlerKey => "toHandlerKey",
        RuntimeHelper::Vue3PushScopeId => "pushScopeId",
        RuntimeHelper::Vue3PopScopeId => "popScopeId",
        RuntimeHelper::Vue3Unref => "unref",
        RuntimeHelper::Vue3IsRef => "isRef",
        RuntimeHelper::Vue3VModelRadio => "vModelRadio",
        RuntimeHelper::Vue3VModelCheckbox => "vModelCheckbox",
        RuntimeHelper::Vue3VModelText => "vModelText",
        RuntimeHelper::Vue3VModelSelect => "vModelSelect",
        RuntimeHelper::Vue3VModelDynamic => "vModelDynamic",
        RuntimeHelper::Vue3WithModifiers => "withModifiers",
        RuntimeHelper::Vue3WithKeys => "withKeys",
        RuntimeHelper::Vue3VShow => "vShow",
        RuntimeHelper::Vue3SsrInterpolate => "ssrInterpolate",
        RuntimeHelper::Vue3SsrRenderVNode => "ssrRenderVNode",
        RuntimeHelper::Vue3SsrRenderComponent => "ssrRenderComponent",
        RuntimeHelper::Vue3SsrRenderSlot => "ssrRenderSlot",
        RuntimeHelper::Vue3SsrRenderSlotInner => "ssrRenderSlotInner",
        RuntimeHelper::Vue3SsrRenderClass => "ssrRenderClass",
        RuntimeHelper::Vue3SsrRenderStyle => "ssrRenderStyle",
        RuntimeHelper::Vue3SsrRenderAttrs => "ssrRenderAttrs",
        RuntimeHelper::Vue3SsrRenderAttr => "ssrRenderAttr",
        RuntimeHelper::Vue3SsrRenderDynamicAttr => "ssrRenderDynamicAttr",
        RuntimeHelper::Vue3SsrRenderList => "ssrRenderList",
        RuntimeHelper::Vue3SsrIncludeBooleanAttr => "ssrIncludeBooleanAttr",
        RuntimeHelper::Vue3SsrLooseEqual => "ssrLooseEqual",
        RuntimeHelper::Vue3SsrLooseContain => "ssrLooseContain",
        RuntimeHelper::Vue3SsrRenderDynamicModel => "ssrRenderDynamicModel",
        RuntimeHelper::Vue3SsrGetDynamicModelProps => "ssrGetDynamicModelProps",
        RuntimeHelper::Vue3SsrRenderTeleport => "ssrRenderTeleport",
        RuntimeHelper::Vue3SsrRenderSuspense => "ssrRenderSuspense",
        RuntimeHelper::Vue3SsrGetDirectiveProps => "ssrGetDirectiveProps",
    }
}
