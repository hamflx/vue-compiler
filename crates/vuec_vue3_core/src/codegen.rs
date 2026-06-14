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

pub(crate) fn sort_helpers_by_order(helpers: &mut Vec<RuntimeHelper>, order: &[RuntimeHelper]) {
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

/// Builds a render source map for generated code and a Vue 3 AST.
pub fn source_map_for_render(
    code: &str,
    ast: &Vue3Ast,
    source: &TemplateSource,
    options: &Vue3CompilerOptions,
) -> Option<SourceMapArtifact> {
    let root = ast.node(ast.root)?;
    let source_name = if source.filename.is_empty() {
        "template.vue.html".to_string()
    } else {
        source.filename.clone()
    };
    let mut names = Vec::new();
    let mut segments = Vec::new();
    let source_map_source = options
        .source_map_source
        .as_deref()
        .unwrap_or(&source.source);
    let source_map_base_offset = if options.source_map_source.is_some() {
        options.source_map_base_offset
    } else {
        source.base_offset
    };
    collect_source_map_segments(
        code,
        ast,
        &root.children,
        source_map_base_offset,
        source_map_source,
        options,
        &mut names,
        &mut segments,
    );
    if segments.is_empty() {
        return None;
    }
    segments.sort_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index.unwrap_or(usize::MAX),
        )
    });
    segments.dedup_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index,
        )
    });
    Some(SourceMapArtifact::from_segments(
        None,
        source_name,
        source_map_source.to_string(),
        names,
        segments,
    ))
}

pub(crate) fn collect_source_map_segments(
    code: &str,
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let mut cursor = 0usize;
    for child_id in children {
        collect_node_source_map(
            code,
            ast,
            *child_id,
            base_offset,
            source,
            options,
            names,
            segments,
            &mut cursor,
        );
    }
}

pub(crate) fn collect_node_source_map(
    code: &str,
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3AstKind::Element(element) => {
            add_vnode_mapping(code, node, base_offset, source, segments, cursor);
            add_element_prop_mappings(code, element, base_offset, source, options, names, segments);
            for child_id in &node.children {
                collect_node_source_map(
                    code,
                    ast,
                    *child_id,
                    base_offset,
                    source,
                    options,
                    names,
                    segments,
                    cursor,
                );
            }
        }
        Vue3AstKind::Interpolation(_) => {
            add_interpolation_mapping(
                code,
                node,
                base_offset,
                source,
                options,
                names,
                segments,
                cursor,
            );
        }
        _ => {}
    }
}

pub(crate) fn add_vnode_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(span) = node.span.source() else {
        return;
    };
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(start) = loc_for_offset(source, local_start) else {
        return;
    };
    let Some(end) = loc_for_offset(source, local_end) else {
        return;
    };
    let tag = match &node.kind {
        Vue3AstKind::Element(element) => &element.tag,
        _ => return,
    };
    let block_needle = format!("_createElementBlock(\"{tag}\"");
    let vnode_needle = format!("_createElementVNode(\"{tag}\"");
    let block_offset = find_code_offset(code, &block_needle, *cursor);
    let vnode_offset = find_code_offset(code, &vnode_needle, *cursor);
    let helper_offset = match (block_offset, vnode_offset) {
        (Some(block), Some(vnode)) => block.min(vnode),
        (Some(block), None) => block,
        (None, Some(vnode)) => vnode,
        (None, None) => return,
    };
    if let Some((line, column)) = loc_for_offset(code, helper_offset) {
        segments.push(SourceMapSegment {
            generated_line: line,
            generated_column: column,
            original_line: start.0,
            original_column: start.1,
            name_index: None,
        });
        let tag_needle = format!("\"{tag}\"");
        if let Some(tag_offset) = find_code_offset(code, &tag_needle, helper_offset) {
            if let Some((end_line, end_column)) = loc_for_offset(code, tag_offset) {
                segments.push(SourceMapSegment {
                    generated_line: end_line,
                    generated_column: end_column,
                    original_line: end.0,
                    original_column: end.1,
                    name_index: None,
                });
                *cursor = tag_offset + tag_needle.len();
            }
        } else {
            *cursor = helper_offset + block_needle.len().min(vnode_needle.len());
        }
    }
}

pub(crate) fn add_interpolation_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Vue3AstKind::Interpolation(interpolation) = &node.kind else {
        return;
    };
    let Some(span) = node.span.source() else {
        return;
    };
    let generated_expression = interpolation.expression.source_string();
    let Some((original_expression, original_start)) =
        original_interpolation_expression(source, span, base_offset, options)
    else {
        return;
    };
    add_expression_token_mappings(
        code,
        source,
        original_expression,
        original_start,
        *cursor,
        uses_prefixed_identifiers(options),
        names,
        segments,
    );
    if let Some(offset) =
        find_code_offset(code, generated_expression.trim(), *cursor).or_else(|| {
            find_code_offset(
                code,
                &format!("_ctx.{}", generated_expression.trim()),
                *cursor,
            )
        })
    {
        *cursor = offset + generated_expression.trim().len();
    }
}

pub(crate) fn original_interpolation_expression<'a>(
    source: &'a str,
    span: Span,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Option<(&'a str, usize)> {
    let (local_start, local_end) = local_source_span_range(source, span, base_offset)?;
    let node_source = source.get(local_start..local_end)?;
    let (open_delimiter, close_delimiter) = options
        .delimiters
        .as_ref()
        .map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    if open_delimiter.is_empty() || close_delimiter.is_empty() {
        return None;
    }
    let open_start = node_source.find(open_delimiter)?;
    let expression_start = local_start + open_start + open_delimiter.len();
    let expression_end = expression_start
        + source
            .get(expression_start..local_end)?
            .find(close_delimiter)?;
    trimmed_source_range(source, expression_start, expression_end)
}

pub(crate) fn original_expression_from_span(
    source: &str,
    span: Span,
    base_offset: usize,
) -> Option<(&str, usize)> {
    let (local_start, local_end) = local_source_span_range(source, span, base_offset)?;
    trimmed_source_range(source, local_start, local_end)
}

pub(crate) fn local_source_span_range(
    source: &str,
    span: Span,
    base_offset: usize,
) -> Option<(usize, usize)> {
    let start = span.start.0.checked_sub(base_offset)?;
    let end = span.end.0.checked_sub(base_offset)?;
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return None;
    }
    Some((start, end))
}

pub(crate) fn trimmed_source_range(
    source: &str,
    start: usize,
    end: usize,
) -> Option<(&str, usize)> {
    let start = trim_start_offset(source, start, end);
    let end = trim_end_offset(source, start, end);
    Some((source.get(start..end)?, start))
}

pub(crate) fn add_element_prop_mappings(
    code: &str,
    element: &Vue3Element,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(attr) => {
                if let Some(span) = attr.name_span {
                    add_direct_mapping(
                        code,
                        source,
                        &attr.name,
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
                if let (Some(value), Some(span)) = (&attr.value, attr.value_span) {
                    add_direct_mapping(
                        code,
                        source,
                        &quote_string(value),
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
            }
            Vue3Prop::Directive(dir) => {
                if dir.name == "bind" {
                    if dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
                    {
                        if let Some(arg_span) = dir.arg_span {
                            add_direct_mapping(
                                code,
                                source,
                                "class:",
                                arg_span.start.0.saturating_sub(base_offset),
                                0,
                                None,
                                segments,
                            );
                        }
                    }
                    add_directive_expression_token_mappings(
                        code,
                        source,
                        dir,
                        base_offset,
                        options,
                        names,
                        segments,
                    );
                }
                if dir.name == "on" && dir.arg.is_some() {
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        let fallback_start = span.start.0.saturating_sub(base_offset);
                        let (original_expression, original_start) =
                            original_expression_from_span(source, span, base_offset)
                                .unwrap_or((expression.trim(), fallback_start));
                        add_event_handler_token_mappings(
                            code,
                            source,
                            original_expression,
                            original_start,
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
                if matches!(dir.name.as_str(), "if" | "else-if" | "for") {
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        let fallback_start = span.start.0.saturating_sub(base_offset);
                        let (original_expression, original_start) =
                            original_expression_from_span(source, span, base_offset)
                                .unwrap_or((expression.trim(), fallback_start));
                        add_expression_token_mappings(
                            code,
                            source,
                            original_expression,
                            original_start,
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn add_directive_expression_token_mappings(
    code: &str,
    source: &str,
    dir: &Vue3Directive,
    base_offset: usize,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) else {
        return;
    };
    let expression = exp.source_string();
    let fallback_start = span.start.0.saturating_sub(base_offset);
    let (original_expression, original_start) =
        original_expression_from_span(source, span, base_offset)
            .unwrap_or((expression.trim(), fallback_start));
    add_expression_token_mappings(
        code,
        source,
        original_expression,
        original_start,
        0,
        uses_prefixed_identifiers(options),
        names,
        segments,
    );
}

pub(crate) fn add_direct_mapping(
    code: &str,
    source: &str,
    generated_needle: &str,
    original_offset: usize,
    generated_from: usize,
    name: Option<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let Some(generated_offset) = find_code_offset(code, generated_needle, generated_from) else {
        return;
    };
    let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset) else {
        return;
    };
    let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
        return;
    };
    let name_index = name.map(|_| 0);
    segments.push(SourceMapSegment {
        generated_line,
        generated_column,
        original_line,
        original_column,
        name_index,
    });
}

pub(crate) fn add_expression_token_mappings(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    add_expression_token_mappings_with_options(
        code,
        source,
        expression,
        original_expression_start,
        generated_from,
        precise_members,
        false,
        names,
        segments,
    );
}

pub(crate) fn add_event_handler_token_mappings(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    add_expression_token_mappings_with_options(
        code,
        source,
        expression,
        original_expression_start,
        generated_from,
        precise_members,
        true,
        names,
        segments,
    );
}

pub(crate) fn add_expression_token_mappings_with_options(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    include_globals: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let tokens = expression_source_map_tokens(expression, include_globals);
    for token in tokens.iter().copied() {
        let generated_needles = if uses_ctx_prefix_for_generated(code, token) {
            vec![format!("_ctx.{token}"), token.to_string()]
        } else {
            vec![token.to_string(), format!("_ctx.{token}")]
        };
        let generated_offset = generated_needles
            .iter()
            .find_map(|needle| find_code_offset(code, needle, generated_from));
        let Some(generated_offset) = generated_offset else {
            continue;
        };
        let Some(original_relative) = expression.find(token) else {
            continue;
        };
        let original_offset = if precise_members || !is_member_tail_token(expression, token) {
            original_expression_start + original_relative
        } else {
            original_expression_start
        };
        let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset)
        else {
            continue;
        };
        let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
            continue;
        };
        let name_index = Some(name_index(names, token));
        segments.push(SourceMapSegment {
            generated_line,
            generated_column,
            original_line,
            original_column,
            name_index,
        });
    }
    if tokens.len() == 1 {
        let token = tokens[0];
        let generated_needles = if uses_ctx_prefix_for_generated(code, token) {
            vec![format!("_ctx.{token}"), token.to_string()]
        } else {
            vec![token.to_string(), format!("_ctx.{token}")]
        };
        if let Some((generated_offset, generated_len)) =
            generated_needles.iter().find_map(|needle| {
                find_code_offset(code, needle, generated_from).map(|offset| (offset, needle.len()))
            })
        {
            add_expression_end_mapping(
                code,
                source,
                generated_offset + generated_len,
                original_expression_start + expression.len(),
                segments,
            );
        }
    }
}

pub(crate) fn add_expression_end_mapping(
    code: &str,
    source: &str,
    generated_offset: usize,
    original_offset: usize,
    segments: &mut Vec<SourceMapSegment>,
) {
    let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset) else {
        return;
    };
    let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
        return;
    };
    segments.push(SourceMapSegment {
        generated_line,
        generated_column,
        original_line,
        original_column,
        name_index: None,
    });
}

pub(crate) fn uses_ctx_prefix_for_generated(code: &str, token: &str) -> bool {
    code.contains(&format!("_ctx.{token}"))
}

pub(crate) fn is_member_tail_token(expression: &str, token: &str) -> bool {
    expression
        .match_indices(token)
        .any(|(index, _)| index > 0 && expression[..index].ends_with('.'))
}

pub(crate) fn expression_source_map_tokens(expression: &str, include_globals: bool) -> Vec<&str> {
    let mut tokens = Vec::new();
    for (index, ch) in expression.char_indices() {
        if !is_identifier_start(ch) {
            continue;
        }
        if index > 0
            && expression[..index]
                .chars()
                .last()
                .is_some_and(is_identifier_continue)
        {
            continue;
        }
        let end = expression[index + ch.len_utf8()..]
            .char_indices()
            .find_map(|(offset, current)| {
                (!is_identifier_continue(current)).then_some(index + ch.len_utf8() + offset)
            })
            .unwrap_or(expression.len());
        let token = &expression[index..end];
        if !is_keyword(token) && (include_globals || !is_global_or_literal(token)) {
            tokens.push(token);
        }
    }
    if tokens.is_empty() && !expression.is_empty() {
        tokens.push(expression);
    }
    tokens
}

pub(crate) fn name_index(names: &mut Vec<String>, name: &str) -> usize {
    if let Some(index) = names.iter().position(|existing| existing == name) {
        index
    } else {
        names.push(name.to_string());
        names.len() - 1
    }
}

pub(crate) fn loc_for_offset(source: &str, offset: usize) -> Option<(u32, u32)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0u32;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < offset {
        match bytes[index] {
            b'\r' => {
                if index + 1 < offset && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                line_start = index + 1;
            }
            b'\n' => {
                line += 1;
                line_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    let column = source[line_start..offset].encode_utf16().count() as u32;
    Some((line, column))
}

pub(crate) fn find_code_offset(code: &str, needle: &str, from: usize) -> Option<usize> {
    code.get(from..)?.find(needle).map(|offset| from + offset)
}

pub(crate) fn render_root_expr(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if options.hoist_static && options.stringify_static {
        if let Some(root_static_call) =
            render_root_static_vnode_cache(ast, children, options, scope)
        {
            return render_cached_single_child(root_static_call, memo_index.alloc());
        }
    }
    let visible = visible_child_ids(ast, children);
    match visible.as_slice() {
        [] => "null".into(),
        [single]
            if children == [*single]
                && root_single_visible_child_uses_direct_codegen(ast, *single) =>
        {
            render_node_expr_scoped(
                ast,
                *single,
                options,
                NodeRenderMode::Root,
                scope,
                memo_index,
            )
        }
        _ => {
            let rendered = render_root_child_sequence(ast, children, options, scope, memo_index);
            format!(
                "(_openBlock(), _createElementBlock(_Fragment, null, {}, {}))",
                render_array(&rendered),
                public_patch_flag_text(root_fragment_patch_flag_ast(ast, children) as i32)
            )
        }
    }
}

pub(crate) fn render_root_child_sequence(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    if !scope.disable_stringify_static_chunks && options.hoist_static && options.stringify_static {
        if let Some(rendered) = render_static_vnode_chunked_children(
            ast,
            &child_nodes,
            options,
            scope,
            NodeRenderMode::RootChild,
            memo_index,
        ) {
            return rendered
                .into_iter()
                .map(|item| {
                    if item.contains("_createStaticVNode(") {
                        render_cached_single_child(item, memo_index.alloc())
                    } else {
                        item
                    }
                })
                .collect();
        }
    }
    render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::RootChild,
        scope,
        memo_index,
    )
}

pub(crate) fn root_single_visible_child_uses_direct_codegen(
    ast: &Vue3Ast,
    child_id: vuec_ast::NodeId,
) -> bool {
    ast.node(child_id).is_some_and(|node| match &node.kind {
        Vue3AstKind::Interpolation(_) => true,
        Vue3AstKind::Element(element) => {
            element.tag_type != Vue3ElementType::SlotOutlet
                && directive_by_name(element, "if").is_none()
                && directive_by_name(element, "for").is_none()
        }
        _ => false,
    })
}

pub(crate) fn root_fragment_patch_flag_ast(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> u16 {
    let visible = visible_child_ids(ast, children).len();
    if visible == 1
        && children.iter().any(|child_id| {
            ast.node(*child_id)
                .is_some_and(|child| matches!(child.kind, Vue3AstKind::Comment(_)))
        })
    {
        64 | 2048
    } else {
        64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeRenderMode {
    Root,
    OnceRoot,
    OnceBlockRoot,
    RootChild,
    Child,
    Cached,
}

pub(crate) fn root_like_render_mode(mode: NodeRenderMode) -> bool {
    matches!(
        mode,
        NodeRenderMode::Root | NodeRenderMode::OnceRoot | NodeRenderMode::OnceBlockRoot
    )
}

pub(crate) fn block_render_mode(mode: NodeRenderMode) -> bool {
    matches!(mode, NodeRenderMode::Root | NodeRenderMode::OnceBlockRoot)
}

pub(crate) fn once_children_mode(mode: NodeRenderMode) -> bool {
    matches!(
        mode,
        NodeRenderMode::OnceRoot | NodeRenderMode::OnceBlockRoot
    )
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderScope {
    pub(crate) locals: Vec<String>,
    pub(crate) in_v_once: bool,
    pub(crate) memo_index_overrides: BTreeMap<vuec_ast::NodeId, usize>,
    pub(crate) static_hoists: StaticHoists,
    pub(crate) disable_stringify_static_chunks: bool,
}

impl RenderScope {
    pub(crate) fn with_locals(&self, locals: Vec<String>) -> Self {
        let mut next = self.clone();
        for local in locals {
            if !next.locals.iter().any(|existing| existing == &local) {
                next.locals.push(local);
            }
        }
        next
    }

    pub(crate) fn with_v_once(&self) -> Self {
        let mut next = self.clone();
        next.in_v_once = true;
        next
    }

    pub(crate) fn with_memo_index_override(&self, node_id: vuec_ast::NodeId, index: usize) -> Self {
        let mut next = self.clone();
        next.memo_index_overrides.insert(node_id, index);
        next
    }

    pub(crate) fn with_static_hoists(&self, hoists: StaticHoists) -> Self {
        let mut next = self.clone();
        next.static_hoists = hoists;
        next
    }

    pub(crate) fn without_stringify_static_chunks(&self) -> Self {
        let mut next = self.clone();
        next.disable_stringify_static_chunks = true;
        next
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StaticHoists {
    pub(crate) declarations: Vec<StaticHoistDeclaration>,
    pub(crate) props_by_node: BTreeMap<vuec_ast::NodeId, usize>,
    pub(crate) bindings_by_node_prop: BTreeMap<(vuec_ast::NodeId, usize), usize>,
}

impl StaticHoists {
    pub(crate) fn push_binding(
        &mut self,
        node_id: vuec_ast::NodeId,
        prop_index: usize,
        expression: String,
        reuse_existing: bool,
    ) -> usize {
        if reuse_existing {
            if let Some(index) = self
                .declarations
                .iter()
                .position(|declaration| {
                    matches!(
                        declaration,
                        StaticHoistDeclaration::BindingExpression { expression: existing }
                            if existing == &expression
                    )
                })
                .map(|index| index + 1)
            {
                self.bindings_by_node_prop
                    .insert((node_id, prop_index), index);
                return index;
            }
        }
        self.declarations
            .push(StaticHoistDeclaration::BindingExpression { expression });
        let index = self.declarations.len();
        self.bindings_by_node_prop
            .insert((node_id, prop_index), index);
        index
    }

    pub(crate) fn push_props_object(&mut self, node_id: vuec_ast::NodeId) -> usize {
        self.declarations
            .push(StaticHoistDeclaration::PropsObject { node_id });
        let index = self.declarations.len();
        self.props_by_node.insert(node_id, index);
        index
    }

    pub(crate) fn binding_index(
        &self,
        node_id: vuec_ast::NodeId,
        prop_index: usize,
    ) -> Option<usize> {
        self.bindings_by_node_prop
            .get(&(node_id, prop_index))
            .copied()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum StaticHoistDeclaration {
    PropsObject { node_id: vuec_ast::NodeId },
    BindingExpression { expression: String },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MemoIndex {
    pub(crate) next: usize,
}

impl MemoIndex {
    pub(crate) fn alloc(&mut self) -> usize {
        let index = self.next;
        self.next += 1;
        index
    }

    pub(crate) fn reserve(&mut self) {
        self.next += 1;
    }
}

pub(crate) fn render_node_expr_scoped(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3AstKind::Root(_) => {
            let rendered = render_child_sequence(
                ast,
                &node.children,
                options,
                NodeRenderMode::Root,
                scope,
                memo_index,
            );
            format!("[{}]", rendered.join(", "))
        }
        Vue3AstKind::Text(text) => quote_text(&text.value),
        Vue3AstKind::Interpolation(interpolation) => {
            format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )
        }
        Vue3AstKind::Comment(comment) => {
            format!("_createCommentVNode({})", quote_string(&comment.value))
        }
        Vue3AstKind::Element(element) => {
            if let Some(for_dir) = directive_by_name(element, "for") {
                return render_for_node(
                    ast, node_id, element, for_dir, options, mode, scope, memo_index,
                );
            }
            if directive_by_name(element, "if").is_some() {
                return render_maybe_once_if_chain(
                    ast,
                    &[node_id],
                    &[Vec::new()],
                    options,
                    mode,
                    scope,
                    memo_index,
                );
            }
            if is_else_branch(element) {
                return "null".into();
            }
            render_maybe_memo_element(
                ast, node_id, element, options, mode, scope, None, memo_index,
            )
        }
        _ => "null".into(),
    }
}

pub(crate) fn render_with_v_once(rendered: String, index: usize) -> String {
    format!(
        "_cache[{index}] || (\n  _setBlockTracking(-1, true),\n  (_cache[{index}] = {}).cacheIndex = {index},\n  _setBlockTracking(1),\n  _cache[{index}]\n)",
        indent_after_first_line(&rendered, 2)
    )
}

pub(crate) fn render_plain_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let tag = &element.tag;
    if tag == "slot" {
        return render_slot_outlet(element, options, scope, memo_index);
    }
    if element.tag_type == Vue3ElementType::Component {
        return render_component_element(
            ast, node_id, element, options, mode, scope, branch_key, memo_index,
        );
    }
    let helper = if block_render_mode(mode) {
        "_createElementBlock"
    } else {
        "_createElementVNode"
    };
    let element_scope = if directive_by_name(element, "once").is_some() || scope.in_v_once {
        scope.with_v_once()
    } else {
        scope.clone()
    };
    let element_scope =
        if select_children_include_unstringifiable_option_value(ast, node_id, element)
            || p_children_include_invalid_html_descendant(ast, node_id, element)
        {
            element_scope.without_stringify_static_chunks()
        } else {
            element_scope
        };
    let props = if branch_key.is_none() {
        element_scope
            .static_hoists
            .props_by_node
            .get(&node_id)
            .map(|index| format!("_hoisted_{index}"))
            .unwrap_or_else(|| {
                render_props(
                    node_id,
                    element,
                    options,
                    &element_scope,
                    branch_key,
                    memo_index,
                )
            })
    } else {
        render_props(
            node_id,
            element,
            options,
            &element_scope,
            branch_key,
            memo_index,
        )
    };
    let static_content = render_static_content_directive_child(element, options);
    let children = if let Some(content) = static_content.as_ref() {
        Some(content.clone())
    } else if exact_content_directive(element).is_some() {
        None
    } else {
        ast.node(node_id)
            .map(|node| {
                render_element_children(
                    ast,
                    &node.children,
                    options,
                    mode,
                    &element_scope,
                    memo_index,
                )
            })
            .filter(|children| !children.is_empty())
    };
    let patch_flag = render_patch_flag_text(render_patch_flag_kind(
        ast,
        node_id,
        element,
        options,
        mode,
        &element_scope,
    ));
    let attrs = if props.is_empty() { None } else { Some(props) };
    let args = render_call_args(
        quote_string(tag),
        attrs.as_deref(),
        children.as_deref(),
        patch_flag.as_str(),
        dynamic_props_arg(element, options, &element_scope).as_str(),
    );
    let rendered = if block_render_mode(mode) {
        format!("(_openBlock(), {}({}))", helper, args)
    } else {
        format!("{}({})", helper, args)
    };
    render_with_runtime_directives(rendered, element, options, scope)
}

pub(crate) fn render_maybe_memo_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let Some(memo) = directive_by_name(element, "memo") else {
        if directive_by_name(element, "once").is_some() && !scope.in_v_once {
            let once_index = memo_index.alloc();
            let scope = scope.with_v_once();
            let rendered = render_plain_element(
                ast,
                node_id,
                element,
                options,
                NodeRenderMode::OnceRoot,
                &scope,
                branch_key,
                memo_index,
            );
            return render_with_v_once(rendered, once_index);
        }
        let scope = if scope.in_v_once {
            scope.with_v_once()
        } else {
            scope.clone()
        };
        return render_plain_element(
            ast, node_id, element, options, mode, &scope, branch_key, memo_index,
        );
    };
    let cache_index = scope
        .memo_index_overrides
        .get(&node_id)
        .copied()
        .unwrap_or_else(|| memo_index.alloc());
    let once_index = (directive_by_name(element, "once").is_some() && !scope.in_v_once)
        .then(|| memo_index.alloc());
    let memo_mode = if element.tag_type == Vue3ElementType::Component {
        if once_index.is_some() && matches!(mode, NodeRenderMode::Root) {
            NodeRenderMode::OnceRoot
        } else {
            mode
        }
    } else if once_index.is_some() && matches!(mode, NodeRenderMode::Root) {
        NodeRenderMode::OnceBlockRoot
    } else {
        NodeRenderMode::Root
    };
    let scope = if once_index.is_some() || scope.in_v_once {
        scope.with_v_once()
    } else {
        scope.clone()
    };
    let rendered = render_plain_element(
        ast, node_id, element, options, memo_mode, &scope, branch_key, memo_index,
    );
    let rendered = render_with_memo(memo, rendered, options, &scope, cache_index);
    if let Some(index) = once_index {
        render_with_v_once(rendered, index)
    } else {
        rendered
    }
}

pub(crate) fn render_with_memo(
    memo: &Vue3Directive,
    rendered: String,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
) -> String {
    let expression = memo
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let expression = rewrite_expression_with_scope(&expression, options, scope);
    format!("_withMemo({expression}, () => {rendered}, _cache, {index})")
}

pub(crate) fn render_call_args(
    tag: String,
    props: Option<&str>,
    children: Option<&str>,
    patch_flag: &str,
    dynamic_props: &str,
) -> String {
    let mut args = vec![tag];
    if let Some(props) = props {
        args.push(props.to_string());
    } else if children.is_some() || !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if let Some(children) = children {
        args.push(children.to_string());
    } else if !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if !patch_flag.is_empty() {
        args.push(patch_flag.trim_start_matches(", ").to_string());
    }
    if !dynamic_props.is_empty() {
        args.push(dynamic_props.trim_start_matches(", ").to_string());
    }
    args.join(", ")
}

pub(crate) fn render_component_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let tag = render_component_tag_expr(element, options, scope);
    let props = render_props(node_id, element, options, scope, branch_key, memo_index);
    let attrs = if props.is_empty() { None } else { Some(props) };
    let children = render_component_slots(ast, node_id, options, scope, memo_index);
    let patch_flag = render_patch_flag_text(component_patch_flag_kind(
        ast, node_id, element, options, scope,
    ));
    let helper = if mode == NodeRenderMode::Root {
        "_createBlock"
    } else {
        "_createVNode"
    };
    let args = render_call_args(
        tag,
        attrs.as_deref(),
        children.as_deref(),
        patch_flag.as_str(),
        dynamic_props_arg(element, options, scope).as_str(),
    );
    let rendered = if mode == NodeRenderMode::Root {
        format!("(_openBlock(), {}({}))", helper, args)
    } else {
        format!("{}({})", helper, args)
    };
    render_with_runtime_directives(rendered, element, options, scope)
}

pub(crate) fn render_component_tag_expr(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if let Some(expression) = vue3_dynamic_component_is_expression(element) {
        return format!(
            "_resolveDynamicComponent({})",
            rewrite_expression_with_scope(&expression.source_string(), options, scope)
        );
    }
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        return helper_reference(helper);
    }
    if let Some(expression) = render_direct_setup_or_props_component_tag(&element.tag, options) {
        return expression;
    }
    if let Some(expression) =
        render_namespaced_setup_or_props_component_tag(&element.tag, options, scope)
    {
        return expression;
    }
    component_asset_id(&element.tag)
}

pub(crate) fn render_direct_setup_or_props_component_tag(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    let name = setup_reference_name_for_tag(tag, options)?;
    match options.binding_metadata.get(&name).map(String::as_str) {
        Some("setup-const" | "setup-reactive-const" | "literal-const") if options.inline => {
            Some(name.to_string())
        }
        Some("setup-let" | "setup-ref" | "setup-maybe-ref") if options.inline => {
            Some(format!("_unref({name})"))
        }
        Some("props") if options.inline => {
            Some(format!("_unref(__props[{}])", quote_string(&name)))
        }
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            Some(format!("$setup[{}]", quote_string(&name)))
        }
        Some("props") => Some(format!("_unref($props[{}])", quote_string(&name))),
        _ => None,
    }
}

pub(crate) fn render_namespaced_setup_or_props_component_tag(
    tag: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let (namespace, member) = tag.split_once('.')?;
    if namespace.is_empty() || member.is_empty() {
        return None;
    }
    match options.binding_metadata.get(namespace).map(String::as_str) {
        Some("setup-ref" | "setup-maybe-ref" | "setup-let" | "props" | "props-aliased")
            if options.inline =>
        {
            render_setup_or_props_component_namespace(namespace, options, scope)
                .map(|namespace| format!("{namespace}.{member}"))
        }
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            let namespace = rewrite_identifier_with_scope(namespace, options, scope);
            Some(format!("{namespace}.{member}"))
        }
        _ => None,
    }
}

pub(crate) fn render_setup_or_props_component_namespace(
    namespace: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if !uses_prefixed_identifiers(options) || scope.locals.iter().any(|local| local == namespace) {
        return Some(namespace.to_string());
    }
    match options.binding_metadata.get(namespace).map(String::as_str) {
        Some("setup-ref" | "setup-maybe-ref" | "setup-let") if options.inline => {
            Some(format!("_unref({namespace})"))
        }
        Some("props") if options.inline => {
            Some(format!("_unref(__props[{}])", quote_string(namespace)))
        }
        Some("props-aliased") if options.inline => {
            let source = options
                .props_aliases
                .get(namespace)
                .map_or(namespace, String::as_str);
            Some(format!(
                "_unref({})",
                render_props_access("__props", source)
            ))
        }
        _ => None,
    }
}

pub(crate) fn render_with_runtime_directives(
    vnode: String,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let directives = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "model" => {
                render_model_runtime_directive_arg(element, dir, options, scope)
            }
            Vue3Prop::Directive(dir) if vue3_directive_needs_runtime_asset(&dir.name) => {
                Some(render_runtime_directive_arg(dir, options, scope))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if directives.is_empty() {
        vnode
    } else {
        format!("_withDirectives({vnode}, {})", render_array(&directives))
    }
}

pub(crate) fn render_runtime_directive_arg(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let runtime = if dir.name == "show" {
        "_vShow".to_string()
    } else if let Some(runtime) = render_setup_runtime_directive(&dir.name, options) {
        runtime
    } else {
        directive_asset_id(&dir.name)
    };
    let mut args = vec![runtime];
    if let Some(exp) = dir.exp.as_ref() {
        args.push(rewrite_expression_with_scope(
            &exp.source_string(),
            options,
            scope,
        ));
    } else if dir.arg.is_some() || !dir.modifiers.is_empty() {
        args.push("void 0".into());
    }
    if let Some(arg) = dir.arg.as_ref() {
        let arg = if dir.is_dynamic_arg {
            rewrite_expression_with_scope(&arg.source_string(), options, scope)
        } else {
            quote_string(&arg.source_string())
        };
        args.push(arg);
    } else if !dir.modifiers.is_empty() {
        args.push("void 0".into());
    }
    if !dir.modifiers.is_empty() {
        let modifiers = dir
            .modifiers
            .iter()
            .map(|modifier| format!("{}: true", json_key(modifier)))
            .collect::<Vec<_>>();
        args.push(render_object(&modifiers));
    }
    format!("[{}]", args.join(", "))
}

pub(crate) fn render_setup_runtime_directive(
    name: &str,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    let binding = format!("v-{name}");
    let name = setup_reference_name(&binding, options)?;
    match options.binding_metadata.get(&name).map(String::as_str) {
        Some("setup-const" | "setup-reactive-const" | "literal-const") if options.inline => {
            Some(name.to_string())
        }
        Some("setup-let" | "setup-ref" | "setup-maybe-ref") if options.inline => {
            Some(format!("_unref({name})"))
        }
        Some("props") if options.inline => {
            Some(format!("_unref(__props[{}])", quote_string(&name)))
        }
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            Some(format!("$setup[{}]", quote_string(&name)))
        }
        Some("props") => Some(format!("_unref($props[{}])", quote_string(&name))),
        _ => None,
    }
}

pub(crate) fn render_model_runtime_directive_arg(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if dir.arg.is_some() {
        return None;
    }
    let helper = helper_reference(vue3_dom_model_runtime_helper(vue3_dom_model_kind(element)?));
    let mut expression = dir
        .exp
        .as_ref()
        .map(|exp| {
            rewrite_expression_with_scope_preserve_outer(&exp.source_string(), options, scope)
        })
        .unwrap_or_else(|| "undefined".into());
    if expression.contains('\n') {
        expression = dedent_after_first_line(&expression, 4);
    }
    let mut args = vec![helper, expression];
    if !dir.modifiers.is_empty() {
        args.push("void 0".into());
        let modifiers = dir
            .modifiers
            .iter()
            .map(|modifier| format!("{}: true", json_key(modifier)))
            .collect::<Vec<_>>();
        args.push(render_object(&modifiers));
    }
    Some(format!("[{}]", args.join(", ")))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ComponentSlotAnalysis {
    pub(crate) has_slots: bool,
    pub(crate) has_dynamic_slots: bool,
}

pub(crate) fn analyze_component_slots(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
) -> ComponentSlotAnalysis {
    let Some(node) = ast.node(node_id) else {
        return ComponentSlotAnalysis::default();
    };
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return ComponentSlotAnalysis::default();
    }
    let mut analysis = ComponentSlotAnalysis {
        has_slots: true,
        has_dynamic_slots: false,
    };
    for child in visible {
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "slot").is_some()
                && (directive_by_name(element, "if").is_some()
                    || directive_by_name(element, "for").is_some()
                    || directive_by_name(element, "else").is_some()
                    || directive_by_name(element, "else-if").is_some()
                    || directive_by_name(element, "slot").is_some_and(|slot| slot.is_dynamic_arg))
            {
                analysis.has_dynamic_slots = true;
            }
        }
    }
    analysis
}

pub(crate) fn render_component_slots(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Option<String> {
    let node = ast.node(node_id)?;
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return None;
    }
    let dynamic_slots = visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    });
    if dynamic_slots {
        Some(render_dynamic_component_slots(
            ast, &visible, options, scope, memo_index,
        ))
    } else {
        Some(render_stable_component_slots(
            ast, &visible, options, scope, memo_index,
        ))
    }
}

pub(crate) fn render_stable_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let mut slots = Vec::new();
    let mut default_children = Vec::new();
    for child in children {
        if let Vue3AstKind::Element(element) = &child.kind {
            if let Some(slot) = directive_by_name(element, "slot") {
                slots.push(render_static_slot_property(
                    ast, child.id, element, slot, options, scope, memo_index,
                ));
                continue;
            }
        }
        default_children.push(child.id);
    }
    if !default_children.is_empty() {
        slots.push(render_slot_property(
            "default",
            "()",
            render_slot_children(ast, &default_children, options, scope, memo_index),
        ));
    }
    slots.push("_: 1 /* STABLE */".into());
    render_object(&slots)
}

pub(crate) fn render_dynamic_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let mut dynamic_entries = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let Vue3AstKind::Element(element) = &child.kind else {
            continue;
        };
        let Some(slot) = directive_by_name(element, "slot") else {
            continue;
        };
        if let Some(if_dir) = directive_by_name(element, "if") {
            dynamic_entries.push(render_conditional_dynamic_slot(
                ast, child.id, element, slot, if_dir, options, scope, index, memo_index,
            ));
        } else if let Some(for_dir) = directive_by_name(element, "for") {
            dynamic_entries.push(render_for_dynamic_slot(
                ast, child.id, element, slot, for_dir, options, scope, memo_index,
            ));
        } else {
            dynamic_entries.push(render_dynamic_slot_object(
                ast, child.id, element, slot, options, scope, None, memo_index,
            ));
        }
    }
    format!(
        "_createSlots({{ _: 2 /* DYNAMIC */ }}, {})",
        render_array(&dynamic_entries)
    )
}

pub(crate) fn render_static_slot_property(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let name = slot
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "default".into());
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope, memo_index))
        .unwrap_or_else(|| "[]".into());
    render_slot_property(&name, &params, children)
}

pub(crate) fn render_slot_property(name: &str, params: &str, children: String) -> String {
    format!("{}: _withCtx({params} => {children})", json_key(name))
}

pub(crate) fn render_conditional_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    if_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
    memo_index: &mut MemoIndex,
) -> String {
    let condition = if_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let condition = render_condition(
        &rewrite_expression_with_scope(&condition, options, scope),
        options,
    );
    let slot = render_dynamic_slot_object(
        ast,
        node_id,
        element,
        slot,
        options,
        scope,
        Some(index),
        memo_index,
    );
    format!(
        "{condition}\n  ? {}\n  : undefined",
        indent_after_first_line(&slot, 4)
    )
}

pub(crate) fn render_for_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    for_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let expression = for_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((source, aliases)) = parse_v_for_expression(&expression) else {
        return render_dynamic_slot_object(
            ast, node_id, element, slot, options, scope, None, memo_index,
        );
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(normalize_v_for_aliases(&aliases));
    let params = aliases.join(", ");
    let body = render_dynamic_slot_object(
        ast, node_id, element, slot, options, &scoped, None, memo_index,
    );
    format!(
        "_renderList({source}, ({params}) => {{\n  return {}\n}})",
        indent_after_first_line(&body, 2)
    )
}

pub(crate) fn render_dynamic_slot_object(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let name = slot_name_expression(slot, options, scope);
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope, memo_index))
        .unwrap_or_else(|| "[]".into());
    let mut properties = vec![
        format!("name: {name}"),
        format!("fn: _withCtx({params} => {children})"),
    ];
    if let Some(key) = key {
        properties.push(format!("key: {}", quote_string(&key.to_string())));
    }
    render_object(&properties)
}

pub(crate) fn slot_name_expression(
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let Some(arg) = slot.arg.as_ref() else {
        return quote_string("default");
    };
    let name = arg.source_string();
    if slot.is_dynamic_arg {
        rewrite_expression_with_scope(&name, options, scope)
    } else {
        quote_string(&name)
    }
}

pub(crate) fn render_slot_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        memo_index,
    );
    render_array(&rendered)
}

pub(crate) fn slot_function_scope(scope: &RenderScope, params: &str) -> RenderScope {
    scope.with_locals(extract_slot_params(params))
}

pub(crate) fn extract_slot_params(params: &str) -> Vec<String> {
    let params = params
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let mut output = Vec::new();
    let mut ident = String::new();
    for ch in params.chars() {
        if is_identifier_continue(ch) {
            ident.push(ch);
        } else if !ident.is_empty() {
            output.push(std::mem::take(&mut ident));
        }
    }
    if !ident.is_empty() {
        output.push(ident);
    }
    output
}

pub(crate) fn component_patch_flag_kind(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<i32> {
    let Some(node) = ast.node(node_id) else {
        return None;
    };
    let visible = visible_children(ast, &node.children);
    let mut flag = if has_dynamic_arg_binding(element) {
        16
    } else if has_dynamic_non_key_props(element, options, scope) {
        8
    } else {
        0
    };
    if visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    }) {
        flag |= 1024;
    }
    (flag != 0).then_some(flag)
}

pub(crate) fn visible_children<'a>(
    ast: &'a Vue3Ast,
    children: &[vuec_ast::NodeId],
) -> Vec<&'a vuec_ast::Node<Vue3NodeKind>> {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| match &child.kind {
            Vue3AstKind::Comment(_) => false,
            Vue3AstKind::Text(text) => !text.value.trim().is_empty(),
            _ => true,
        })
        .collect()
}

pub(crate) fn collect_component_tags(ast: &Vue3Ast) -> Vec<String> {
    let mut tags = Vec::new();
    for node in &ast.nodes {
        if let Vue3AstKind::Element(element) = &node.kind {
            if element.tag_type == Vue3ElementType::Component
                && !tags.iter().any(|tag| tag == &element.tag)
            {
                tags.push(element.tag.clone());
            }
        }
    }
    tags
}

pub(crate) fn collect_runtime_directive_names(ast: &Vue3Ast) -> Vec<String> {
    let mut directives = Vec::new();
    for node in &ast.nodes {
        if let Vue3AstKind::Element(element) = &node.kind {
            for prop in &element.props {
                let Vue3Prop::Directive(dir) = prop else {
                    continue;
                };
                if dir.name != "show"
                    && vue3_directive_needs_runtime_asset(&dir.name)
                    && !directives.iter().any(|existing| existing == &dir.name)
                {
                    directives.push(dir.name.clone());
                }
            }
        }
    }
    directives
}

pub(crate) fn collect_vue3_component_asset(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    components: &mut BTreeSet<String>,
    helpers: &mut BTreeSet<RuntimeHelper>,
) {
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        helpers.insert(helper);
    } else if !vue3_tag_is_direct_setup_or_props_reference(&element.tag, options)
        && !matches!(element.tag.as_str(), "component" | "Component")
        && !vue3_tag_is_namespaced_setup_or_props_reference(&element.tag, options)
    {
        components.insert(element.tag.clone());
    }
    if matches!(element.tag.as_str(), "component" | "Component")
        || vue3_dynamic_component_is_expression(element).is_some()
    {
        helpers.insert(RuntimeHelper::Vue3ResolveDynamicComponent);
    }
}

pub(crate) fn vue3_tag_is_direct_setup_or_props_reference(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    setup_reference_name_for_tag(tag, options).is_some()
}

pub(crate) fn vue3_tag_is_namespaced_setup_or_props_reference(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    let Some((namespace, member)) = tag.split_once('.') else {
        return false;
    };
    !namespace.is_empty()
        && !member.is_empty()
        && options.binding_metadata.get(namespace).is_some_and(|kind| {
            matches!(
                kind.as_str(),
                "setup-ref"
                    | "setup-maybe-ref"
                    | "setup-let"
                    | "setup-const"
                    | "setup-reactive-const"
                    | "literal-const"
                    | "props"
                    | "props-aliased"
            )
        })
}

pub(crate) fn collect_vue3_runtime_directive_asset(
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    directives: &mut BTreeSet<String>,
    helpers: &mut BTreeSet<RuntimeHelper>,
) {
    if !vue3_directive_needs_runtime_asset(&directive.name) {
        return;
    }
    if directive.name == "show" {
        helpers.insert(RuntimeHelper::Vue3VShow);
    } else if directive.name == "model" {
        helpers.insert(RuntimeHelper::Vue3VModelDynamic);
    } else if setup_reference_name(&format!("v-{}", directive.name), options).is_some() {
        if render_setup_runtime_directive(&directive.name, options)
            .is_some_and(|runtime| runtime.contains("_unref("))
        {
            helpers.insert(RuntimeHelper::Vue3Unref);
        }
    } else {
        directives.insert(directive.name.clone());
        helpers.insert(RuntimeHelper::Vue3ResolveDirective);
    }
    helpers.insert(RuntimeHelper::Vue3WithDirectives);
}

pub(crate) fn collect_vue3_binding_rewrite_helpers(
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    helpers: &mut BTreeSet<RuntimeHelper>,
) {
    if !uses_prefixed_identifiers(options) {
        return;
    }
    let source = directive
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    if source.trim().is_empty() {
        return;
    }
    let rewritten = if directive.name == "on" && directive.arg.is_some() {
        let scope = RenderScope::default();
        rewrite_handler_expression_with_scope(&source, options, &scope)
    } else {
        rewrite_js_like_expression(&source, options)
    };
    for helper in vue3_for_helpers_for_content(&rewritten) {
        match helper {
            "UNREF" => {
                helpers.insert(RuntimeHelper::Vue3Unref);
            }
            "IS_REF" => {
                helpers.insert(RuntimeHelper::Vue3IsRef);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_core_component_runtime_helper(tag: &str) -> Option<RuntimeHelper> {
    match tag {
        "Teleport" | "teleport" => Some(RuntimeHelper::Vue3Teleport),
        "Suspense" | "suspense" => Some(RuntimeHelper::Vue3Suspense),
        "KeepAlive" | "keep-alive" => Some(RuntimeHelper::Vue3KeepAlive),
        "BaseTransition" | "base-transition" => Some(RuntimeHelper::Vue3BaseTransition),
        "Transition" | "transition" => Some(RuntimeHelper::Vue3Transition),
        "TransitionGroup" | "transition-group" => Some(RuntimeHelper::Vue3TransitionGroup),
        _ => None,
    }
}

pub(crate) fn vue3_dynamic_component_is_expression(
    element: &Vue3Element,
) -> Option<&Vue3Expression> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "is") =>
        {
            dir.exp.as_ref()
        }
        _ => None,
    })
}

pub(crate) fn vue3_directive_needs_runtime_asset(name: &str) -> bool {
    !matches!(
        name,
        "bind"
            | "cloak"
            | "else"
            | "else-if"
            | "for"
            | "html"
            | "if"
            | "memo"
            | "model"
            | "on"
            | "once"
            | "pre"
            | "slot"
            | "text"
    )
}

pub(crate) fn component_asset_id(tag: &str) -> String {
    to_valid_asset_id(tag, "component")
}

pub(crate) fn to_valid_asset_id(name: &str, asset_type: &str) -> String {
    format!("_{asset_type}_{}", to_valid_asset_part(name))
}

pub(crate) fn to_valid_asset_part(value: &str) -> String {
    let mut output = String::new();
    for unit in value.encode_utf16() {
        match unit {
            65..=90 | 97..=122 | 48..=57 | 95 => {
                let ch = char::from_u32(unit as u32).unwrap_or('_');
                output.push(ch);
            }
            45 => output.push('_'),
            _ => output.push_str(&unit.to_string()),
        }
    }
    output
}

pub(crate) fn render_slot_outlet(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let name = element
        .props
        .iter()
        .find_map(|prop| match prop {
            Vue3Prop::Attribute(attr) if attr.name == "name" => attr.value.as_deref(),
            _ => None,
        })
        .map(quote_string)
        .or_else(|| {
            directive_by_name(element, "bind").and_then(|dir| {
                if dir.is_dynamic_arg {
                    return None;
                }
                let arg = dir.arg.as_ref()?.source_string();
                (arg == "name").then(|| {
                    rewrite_expression_with_scope(
                        &dir.exp
                            .as_ref()
                            .map(Vue3Expression::source_string)
                            .unwrap_or_default(),
                        options,
                        scope,
                    )
                })
            })
        })
        .unwrap_or_else(|| quote_string("default"));
    let slots = if options.prefix_identifiers || options.mode == "module" {
        "_ctx.$slots"
    } else {
        "$slots"
    };
    let props = render_slot_outlet_props(element, options, scope, memo_index);
    if props.is_empty() {
        format!("_renderSlot({}, {})", slots, name)
    } else {
        format!("_renderSlot({}, {}, {})", slots, name, props)
    }
}

pub(crate) fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    if options.hoist_static
        && once_children_mode(parent_mode)
        && child_nodes.len() == 1
        && child_nodes
            .first()
            .is_some_and(|child| is_static_element_tree_for_cache(ast, child))
    {
        let rendered = render_node_expr_scoped(
            ast,
            child_nodes[0].id,
            options,
            NodeRenderMode::Cached,
            scope,
            memo_index,
        );
        return render_array(&[render_cached_single_child(rendered, memo_index.alloc())]);
    }
    if !scope.disable_stringify_static_chunks
        && options.hoist_static
        && options.stringify_static
        && root_like_render_mode(parent_mode)
        && !should_cache_children(ast, &child_nodes)
        && should_stringify_static_children(&child_nodes)
    {
        if let Some(static_call) = render_static_vnode_cache(ast, &child_nodes, options, scope) {
            return render_cached_children_array(vec![static_call], memo_index.alloc(), false);
        }
    }
    if options.hoist_static
        && root_like_render_mode(parent_mode)
        && should_cache_children(ast, &child_nodes)
        && !child_nodes
            .iter()
            .any(|child| static_tree_contains_comment(ast, child))
    {
        if options.stringify_static && !scope.disable_stringify_static_chunks {
            if let Some(static_call) = render_static_vnode_cache(ast, &child_nodes, options, scope)
            {
                return render_cached_children_array(vec![static_call], memo_index.alloc(), false);
            }
            if let Some(rendered) = render_static_vnode_chunked_children(
                ast,
                &child_nodes,
                options,
                scope,
                NodeRenderMode::Cached,
                memo_index,
            ) {
                return render_cached_children_array(
                    rendered,
                    memo_index.alloc(),
                    !options.stringify_static,
                );
            }
        }
        let rendered = child_nodes
            .iter()
            .map(|child| {
                render_node_expr_scoped(
                    ast,
                    child.id,
                    options,
                    NodeRenderMode::Cached,
                    scope,
                    memo_index,
                )
            })
            .collect::<Vec<_>>();
        if !rendered.is_empty() {
            return render_cached_children_array(
                rendered,
                memo_index.alloc(),
                !options.stringify_static,
            );
        }
    }
    if !scope.disable_stringify_static_chunks
        && options.hoist_static
        && options.stringify_static
        && (root_like_render_mode(parent_mode) || once_children_mode(parent_mode))
    {
        if let Some(rendered) = render_static_vnode_chunked_children(
            ast,
            &child_nodes,
            options,
            scope,
            NodeRenderMode::Child,
            memo_index,
        ) {
            let cache_static_chunks = once_children_mode(parent_mode)
                || rendered
                    .iter()
                    .any(|item| item.contains("_setBlockTracking(-1, true)"));
            let rendered = if cache_static_chunks {
                rendered
                    .into_iter()
                    .map(|item| {
                        if item.contains("_createStaticVNode(") {
                            render_cached_single_child(item, memo_index.alloc())
                        } else {
                            item
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                rendered
            };
            return render_array(&rendered);
        }
    }
    if !scope.disable_stringify_static_chunks
        && options.hoist_static
        && options.stringify_static
        && once_children_mode(parent_mode)
        && !child_nodes.iter().all(|child| {
            matches!(
                child.kind,
                Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
            )
        })
    {
        let rendered = render_child_sequence_or_static_cache(
            ast,
            children,
            options,
            NodeRenderMode::Child,
            scope,
            memo_index,
            true,
        );
        if !rendered.is_empty() {
            return render_array(&rendered);
        }
    }
    if child_nodes.iter().all(|child| {
        matches!(
            child.kind,
            Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
        )
    }) {
        if once_children_mode(parent_mode) {
            return render_array(&[render_text_vnode(ast, children, options, scope)]);
        }
        return render_text_sequence_expr(ast, children, options, scope);
    }
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        memo_index,
    );
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && child_nodes.first().is_some_and(|child| is_text_like(child))
        && !root_like_render_mode(parent_mode)
    {
        rendered.into_iter().next().unwrap()
    } else {
        render_array(&rendered)
    }
}

pub(crate) fn render_child_sequence_or_static_cache(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
    cache_static_chunks: bool,
) -> Vec<String> {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    if options.hoist_static && options.stringify_static {
        if let Some(rendered) = render_static_vnode_chunked_children(
            ast,
            &child_nodes,
            options,
            scope,
            mode,
            memo_index,
        ) {
            return rendered
                .into_iter()
                .map(|item| {
                    if cache_static_chunks && item.contains("_createStaticVNode(") {
                        render_cached_single_child(item, memo_index.alloc())
                    } else {
                        item
                    }
                })
                .collect();
        }
    }
    render_child_sequence(ast, children, options, mode, scope, memo_index)
}

pub(crate) fn render_cached_children_array(
    rendered: Vec<String>,
    cache_index: usize,
    compact_single_vnode: bool,
) -> String {
    if compact_single_vnode
        && rendered
            .first()
            .is_some_and(|item| !item.contains("_createStaticVNode("))
    {
        if let [single] = rendered.as_slice() {
            return format!("[...(_cache[{cache_index}] || (_cache[{cache_index}] = [{single}]))]");
        }
    }
    if rendered
        .iter()
        .any(|item| item.contains("_createStaticVNode("))
    {
        return format!(
            "[...(_cache[{cache_index}] || (_cache[{cache_index}] = {}))]",
            render_array(&rendered)
        );
    }
    format!(
        "[...(_cache[{cache_index}] || (_cache[{cache_index}] = {}))]",
        render_array(&rendered)
    )
}

pub(crate) fn render_cached_single_child(rendered: String, cache_index: usize) -> String {
    format!("_cache[{cache_index}] || (_cache[{cache_index}] = {rendered})")
}

pub(crate) fn render_child_sequence(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let mut rendered = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        if !scope.disable_stringify_static_chunks
            && options.hoist_static
            && options.stringify_static
        {
            let remaining_nodes = children[index..]
                .iter()
                .filter_map(|child_id| ast.node(*child_id))
                .collect::<Vec<_>>();
            if let Some(chunks) = render_static_vnode_chunked_children(
                ast,
                &remaining_nodes,
                options,
                scope,
                mode,
                memo_index,
            ) {
                for item in chunks {
                    rendered.push(if item.contains("_createStaticVNode(") {
                        render_cached_single_child(item, memo_index.alloc())
                    } else {
                        item
                    });
                }
                break;
            }
        }
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if options.hoist_static
            && mode == NodeRenderMode::RootChild
            && is_static_element_tree_for_cache(ast, child)
            && !static_tree_contains_comment(ast, child)
        {
            rendered.push(render_static_element_cache(
                ast, child.id, options, scope, memo_index,
            ));
            index += 1;
            continue;
        }
        if is_text_like(child) {
            let start = index;
            index += 1;
            while index < children.len()
                && ast
                    .node(children[index])
                    .is_some_and(|candidate| is_text_like(candidate))
            {
                index += 1;
            }
            rendered.push(render_text_vnode(
                ast,
                &children[start..index],
                options,
                scope,
            ));
            continue;
        }
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "if").is_some() {
                let mut branch_ids = vec![child_id];
                let mut branch_comment_ids: Vec<Vec<vuec_ast::NodeId>> = vec![Vec::new()];
                let mut pending_comment_ids = Vec::new();
                index += 1;
                while index < children.len() {
                    let Some(candidate) = ast.node(children[index]) else {
                        index += 1;
                        continue;
                    };
                    if matches!(candidate.kind, Vue3AstKind::Comment(_)) {
                        pending_comment_ids.push(children[index]);
                        index += 1;
                        continue;
                    }
                    if let Vue3AstKind::Element(candidate_element) = &candidate.kind {
                        if is_else_branch(candidate_element) {
                            branch_ids.push(children[index]);
                            branch_comment_ids.push(std::mem::take(&mut pending_comment_ids));
                            index += 1;
                            continue;
                        }
                    }
                    break;
                }
                rendered.push(render_maybe_once_if_chain(
                    ast,
                    &branch_ids,
                    &branch_comment_ids,
                    options,
                    mode,
                    scope,
                    memo_index,
                ));
                continue;
            }
            if is_else_branch(element) {
                index += 1;
                continue;
            }
        }
        rendered.push(render_node_expr_scoped(
            ast, child_id, options, mode, scope, memo_index,
        ));
        index += 1;
    }
    rendered
}

pub(crate) fn is_static_element_tree_for_cache(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
) -> bool {
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    if element.tag == "slot"
        || element.tag_type != Vue3ElementType::Element
        || !element
            .props
            .iter()
            .all(vue3_prop_is_vnode_cacheable_static)
    {
        return false;
    }
    node.children.iter().all(|child_id| {
        ast.node(*child_id).is_some_and(|child| match &child.kind {
            Vue3AstKind::Text(_) | Vue3AstKind::Comment(_) => true,
            Vue3AstKind::Element(_) => is_static_element_tree_for_cache(ast, child),
            _ => false,
        })
    })
}

pub(crate) fn static_tree_contains_comment(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
) -> bool {
    matches!(node.kind, Vue3AstKind::Comment(_))
        || node.children.iter().any(|child_id| {
            ast.node(*child_id)
                .is_some_and(|child| static_tree_contains_comment(ast, child))
        })
}

pub(crate) fn select_children_include_unstringifiable_option_value(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
) -> bool {
    element.tag == "select"
        && element.ns == vuec_ast::HtmlNamespace::Html
        && ast.node(node_id).is_some_and(|node| {
            node.children.iter().any(|child_id| {
                ast.node(*child_id)
                    .is_some_and(|child| option_has_unstringifiable_value_binding(child))
            })
        })
}

pub(crate) fn p_children_include_invalid_html_descendant(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
) -> bool {
    element.tag.eq_ignore_ascii_case("p")
        && element.ns == vuec_ast::HtmlNamespace::Html
        && ast.node(node_id).is_some_and(|node| {
            node.children
                .iter()
                .any(|child_id| static_html_contains_invalid_p_descendant(ast, *child_id))
        })
}

pub(crate) fn option_has_unstringifiable_value_binding(
    node: &vuec_ast::Node<Vue3NodeKind>,
) -> bool {
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    element.tag == "option"
        && element.ns == vuec_ast::HtmlNamespace::Html
        && element.props.iter().any(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "bind" => dir
                .arg
                .as_ref()
                .is_some_and(|arg| arg.source_string() == "value"),
            _ => false,
        })
}

pub(crate) fn render_static_element_cache(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let rendered = render_node_expr_scoped(
        ast,
        node_id,
        options,
        NodeRenderMode::Cached,
        scope,
        memo_index,
    );
    render_cached_single_child(rendered, memo_index.alloc())
}

pub(crate) fn is_text_like(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    matches!(
        node.kind,
        Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
    )
}

pub(crate) fn render_text_sequence_expr(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter_map(|child| match &child.kind {
            Vue3AstKind::Text(text) => Some(quote_text(&text.value)),
            Vue3AstKind::Interpolation(interpolation) => Some(format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

pub(crate) fn render_text_vnode(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = render_text_sequence_expr(ast, children, options, scope);
    let has_interpolation = children.iter().any(|child_id| {
        ast.node(*child_id)
            .is_some_and(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
    });
    if has_interpolation {
        format!("_createTextVNode({}, 1 /* TEXT */)", expression)
    } else {
        format!("_createTextVNode({})", expression)
    }
}

pub(crate) fn render_if_chain(
    ast: &Vue3Ast,
    branch_ids: &[vuec_ast::NodeId],
    branch_comment_ids: &[Vec<vuec_ast::NodeId>],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    fn render_branch(
        ast: &Vue3Ast,
        branch_ids: &[vuec_ast::NodeId],
        branch_comment_ids: &[Vec<vuec_ast::NodeId>],
        index: usize,
        options: &Vue3CompilerOptions,
        mode: NodeRenderMode,
        scope: &RenderScope,
        memo_index: &mut MemoIndex,
    ) -> String {
        let Some(branch_id) = branch_ids.get(index).copied() else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(node) = ast.node(branch_id) else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(element) = (match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        }) else {
            return render_node_expr_scoped(ast, branch_id, options, mode, scope, memo_index);
        };
        let branch_expr = render_if_branch_expr(
            ast,
            branch_id,
            element,
            branch_comment_ids
                .get(index)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            options,
            mode,
            scope,
            index,
            memo_index,
        );
        let condition = if index == 0 {
            directive_by_name(element, "if")
        } else {
            directive_by_name(element, "else-if")
        };
        if let Some(condition) = condition.and_then(|dir| dir.exp.as_ref()) {
            let condition = render_condition(
                &rewrite_expression_with_scope(&condition.source_string(), options, scope),
                options,
            );
            let next_is_else_if = branch_ids
                .get(index + 1)
                .and_then(|branch_id| ast.node(*branch_id))
                .and_then(|node| match &node.kind {
                    Vue3AstKind::Element(element) => Some(element),
                    _ => None,
                })
                .is_some_and(|element| directive_by_name(element, "else-if").is_some());
            let alternate = render_branch(
                ast,
                branch_ids,
                branch_comment_ids,
                index + 1,
                options,
                mode,
                scope,
                memo_index,
            );
            format!(
                "{condition}\n  ? {}\n  : {}",
                indent_after_first_line(&branch_expr, 4),
                indent_after_first_line(&alternate, if next_is_else_if { 2 } else { 4 })
            )
        } else {
            branch_expr
        }
    }
    render_branch(
        ast,
        branch_ids,
        branch_comment_ids,
        0,
        options,
        mode,
        scope,
        memo_index,
    )
}

pub(crate) fn render_maybe_once_if_chain(
    ast: &Vue3Ast,
    branch_ids: &[vuec_ast::NodeId],
    branch_comment_ids: &[Vec<vuec_ast::NodeId>],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if scope.in_v_once {
        return render_if_chain(
            ast,
            branch_ids,
            branch_comment_ids,
            options,
            mode,
            scope,
            memo_index,
        );
    }
    let Some(first_element) = branch_ids
        .first()
        .and_then(|branch_id| ast.node(*branch_id))
        .and_then(|node| match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        })
    else {
        return render_if_chain(
            ast,
            branch_ids,
            branch_comment_ids,
            options,
            mode,
            scope,
            memo_index,
        );
    };
    if directive_by_name(first_element, "once").is_none() {
        return render_if_chain(
            ast,
            branch_ids,
            branch_comment_ids,
            options,
            mode,
            scope,
            memo_index,
        );
    }
    let (once_index, scoped) = if directive_by_name(first_element, "memo").is_some() {
        let memo_slot = memo_index.alloc();
        let once_slot = memo_index.alloc();
        (
            once_slot,
            scope
                .with_v_once()
                .with_memo_index_override(branch_ids[0], memo_slot),
        )
    } else {
        (memo_index.alloc(), scope.with_v_once())
    };
    let rendered = render_if_chain(
        ast,
        branch_ids,
        branch_comment_ids,
        options,
        mode,
        &scoped,
        memo_index,
    );
    render_with_v_once(rendered, once_index)
}

pub(crate) fn render_if_branch_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    leading_comment_ids: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: usize,
    memo_index: &mut MemoIndex,
) -> String {
    if element.tag == "template" {
        let children = ast
            .node(node_id)
            .map(|node| {
                render_fragment_children(
                    ast,
                    &prefixed_child_ids(leading_comment_ids, &node.children),
                    options,
                    scope,
                    memo_index,
                )
            })
            .unwrap_or_else(|| "[]".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {{ key: {branch_key} }}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    if !leading_comment_ids.is_empty() {
        let mut rendered = leading_comment_ids
            .iter()
            .map(|comment_id| {
                render_node_expr_scoped(
                    ast,
                    *comment_id,
                    options,
                    NodeRenderMode::Child,
                    scope,
                    memo_index,
                )
            })
            .collect::<Vec<_>>();
        rendered.push(render_maybe_memo_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Child,
            scope,
            None,
            memo_index,
        ));
        let children = render_array(&rendered);
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {{ key: {branch_key} }}, {children}, 2112 /* STABLE_FRAGMENT, DEV_ROOT_FRAGMENT */))"
        );
    }
    if directive_by_name(element, "once").is_some() {
        return render_maybe_memo_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::OnceBlockRoot,
            scope,
            Some(branch_key),
            memo_index,
        );
    }
    render_maybe_memo_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        Some(branch_key),
        memo_index,
    )
}

pub(crate) fn prefixed_child_ids(
    prefix: &[vuec_ast::NodeId],
    children: &[vuec_ast::NodeId],
) -> Vec<vuec_ast::NodeId> {
    prefix.iter().chain(children.iter()).copied().collect()
}

pub(crate) fn render_fragment_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        memo_index,
    );
    render_array(&rendered)
}

pub(crate) fn render_for_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    _memo_index: &mut MemoIndex,
) -> String {
    let Some(expression) = directive.exp.as_ref().map(Vue3Expression::source_string) else {
        return render_once_plain_fallback(ast, node_id, element, options, scope, _memo_index);
    };
    let parsed = parse_v_for_expression(&expression);
    let Some((source, aliases)) = parsed else {
        return render_once_plain_fallback(ast, node_id, element, options, scope, _memo_index);
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(normalize_v_for_aliases(&aliases));
    let should_wrap_once = directive_by_name(element, "once").is_some() && !scope.in_v_once;
    let once_index = (should_wrap_once && directive_by_name(element, "memo").is_none())
        .then(|| _memo_index.alloc());
    let scoped = if directive_by_name(element, "once").is_some() && !scope.in_v_once {
        scoped.with_v_once()
    } else {
        scoped
    };
    let params = aliases.join(", ");
    let Some(memo) = directive_by_name(element, "memo") else {
        let body = render_v_for_body(ast, node_id, element, options, &scoped, _memo_index);
        let fragment_flag = v_for_fragment_patch_flag(element);
        let body = indent_after_first_line(&body, 2);
        let rendered = format!(
            "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  return {body}\n}}), {fragment_flag}))"
        );
        return once_index.map_or(rendered.clone(), |index| {
            render_with_v_once(rendered, index)
        });
    };
    let cache_index = _memo_index.alloc();
    // Vue's transformMemo reserves a cache slot for v-for memo wrappers even
    // though the emitted render-list memo path only references cache_index.
    _memo_index.reserve();
    let once_index = should_wrap_once.then(|| _memo_index.alloc());
    let body = render_v_for_body(ast, node_id, element, options, &scoped, _memo_index);
    let params = format!("{params}, __, ___, _cached");
    let memo_expression = memo
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let memo_expression = rewrite_expression_with_scope(&memo_expression, options, &scoped);
    let key = v_for_key_expression(element, options, &scoped);
    let guard = key.map_or_else(
        || format!("_cached && _cached.el && _isMemoSame(_cached, _memo)"),
        |key| {
            format!("_cached && _cached.el && _cached.key === {key} && _isMemoSame(_cached, _memo)")
        },
    );
    let body = indent_after_first_line(&body, 2);
    let rendered = format!(
        "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  const _memo = ({memo_expression})\n  if ({guard}) return _cached\n  const _item = {body}\n  _item.memo = _memo\n  return _item\n}}, _cache, {cache_index}), 128 /* KEYED_FRAGMENT */))"
    );
    once_index.map_or(rendered.clone(), |index| {
        render_with_v_once(rendered, index)
    })
}

pub(crate) fn render_once_plain_fallback(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if directive_by_name(element, "once").is_some() && !scope.in_v_once {
        let once_index = memo_index.alloc();
        let scope = scope.with_v_once();
        let rendered = render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::OnceRoot,
            &scope,
            None,
            memo_index,
        );
        render_with_v_once(rendered, once_index)
    } else {
        render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Root,
            scope,
            None,
            memo_index,
        )
    }
}

pub(crate) fn render_v_for_body(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    if element.tag == "template" {
        let Some(node) = ast.node(node_id) else {
            return "null".into();
        };
        let visible = visible_children(ast, &node.children);
        if visible.len() == 1 {
            if let Some(child) = visible.first() {
                if let Vue3AstKind::Element(child_element) = &child.kind {
                    let key = v_for_key_expression(element, options, scope);
                    let body = render_plain_element(
                        ast,
                        child.id,
                        child_element,
                        options,
                        NodeRenderMode::Root,
                        scope,
                        None,
                        memo_index,
                    );
                    return inject_key_into_vnode_call(&body, key.as_deref());
                }
            }
        }
        let key = v_for_key_expression(element, options, scope);
        let children = render_fragment_children(ast, &node.children, options, scope, memo_index);
        let props = key
            .map(|key| format!("{{ key: {key} }}"))
            .unwrap_or_else(|| "null".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {props}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    render_plain_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        None,
        memo_index,
    )
}

pub(crate) fn v_for_fragment_patch_flag(element: &Vue3Element) -> &'static str {
    if v_for_key_expression(
        element,
        &Vue3CompilerOptions::default(),
        &RenderScope::default(),
    )
    .is_some()
    {
        "128 /* KEYED_FRAGMENT */"
    } else {
        "256 /* UNKEYED_FRAGMENT */"
    }
}

pub(crate) fn v_for_key_expression(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "key") =>
        {
            dir.exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .filter(|value| !value.trim().is_empty())
                .map(|value| rewrite_expression_with_scope(&value, options, scope))
        }
        Vue3Prop::Attribute(attr) if attr.name == "key" => {
            attr.value.as_ref().map(|value| quote_string(value))
        }
        _ => None,
    })
}

pub(crate) fn inject_key_into_vnode_call(body: &str, key: Option<&str>) -> String {
    let Some(key) = key else {
        return body.to_string();
    };
    if body.contains(" key: ") || body.contains("{ key:") {
        return body.to_string();
    }
    let Some(start) = body.find("_createElementBlock(") else {
        return body.to_string();
    };
    let args_start = start + "_createElementBlock(".len();
    let Some(first_comma) = find_top_level_comma(body, args_start) else {
        return body.to_string();
    };
    let Some(close) = find_matching_call_close(body, args_start) else {
        return body.to_string();
    };
    if body[first_comma + 1..close].trim().is_empty() {
        let mut output = body.to_string();
        output.insert_str(first_comma, &format!(", {{ key: {key} }}"));
        return output;
    }
    let second_arg_start = first_comma + 1;
    let second_arg_end = find_top_level_comma(body, second_arg_start).unwrap_or(close);
    let second_arg = body[second_arg_start..second_arg_end].trim();
    if second_arg == "null" {
        let mut output = body.to_string();
        output.replace_range(
            second_arg_start..second_arg_end,
            &format!(" {{ key: {key} }}"),
        );
        return output;
    }
    body.to_string()
}

pub(crate) fn find_top_level_comma(value: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let chars = value.char_indices().skip_while(|(index, _)| *index < start);
    for (index, ch) in chars {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            ',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

pub(crate) fn find_matching_call_close(value: &str, start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut quote = None;
    let chars = value.char_indices().skip_while(|(index, _)| *index < start);
    for (index, ch) in chars {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn render_array(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!(
            "[\n{}\n]",
            items
                .iter()
                .map(|item| indent_lines(item, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

pub(crate) fn render_string_array(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| quote_string(item))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn vue3_dom_event_option_postfix(modifiers: &[String]) -> String {
    modifiers
        .iter()
        .map(|modifier| capitalize(modifier))
        .collect()
}

pub(crate) fn indent_lines(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn indent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(&prefix);
        output.push_str(line);
    }
    output
}

pub(crate) fn dedent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(line.strip_prefix(&prefix).unwrap_or(line));
    }
    output
}

pub(crate) fn render_condition(condition: &str, options: &Vue3CompilerOptions) -> String {
    if uses_prefixed_identifiers(options) {
        format!("({condition})")
    } else {
        condition.to_string()
    }
}

pub(crate) fn render_vue3_ssr_slot_condition(condition: String) -> String {
    if condition.starts_with("_ctx.") {
        format!("({condition})")
    } else {
        condition
    }
}

pub(crate) fn should_cache_children(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
) -> bool {
    !children.is_empty()
        && children
            .iter()
            .all(|child| is_static_element_tree_for_cache(ast, child))
}

pub(crate) fn should_stringify_static_children(children: &[&vuec_ast::Node<Vue3NodeKind>]) -> bool {
    !children.is_empty()
        && children
            .iter()
            .all(|child| is_stringifiable_static_node_for_cache(child))
}

pub(crate) fn is_stringifiable_static_node_for_cache(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    match &node.kind {
        Vue3AstKind::Element(element) => {
            element.tag != "slot"
                && element
                    .props
                    .iter()
                    .all(|prop| vue3_prop_is_static_cacheable_for_ns(prop, element.ns))
        }
        Vue3AstKind::Text(_) => true,
        Vue3AstKind::Interpolation(interpolation) => {
            static_const_eval_source(&interpolation.expression.source_string()).is_some()
        }
        _ => false,
    }
}

pub(crate) const STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT: usize = 5;
pub(crate) const STRINGIFY_STATIC_NODE_COUNT: usize = 20;

#[derive(Clone, Debug)]
pub(crate) struct StaticHtmlAnalysis {
    pub(crate) html: StaticHtmlBuffer,
    pub(crate) dom_nodes: usize,
    pub(crate) node_count: usize,
    pub(crate) element_with_binding_count: usize,
}

impl StaticHtmlAnalysis {
    pub(crate) fn append(&mut self, other: StaticHtmlAnalysis) {
        self.html.append(other.html);
        self.dom_nodes += other.dom_nodes;
        self.node_count += other.node_count;
        self.element_with_binding_count += other.element_with_binding_count;
    }

    pub(crate) fn meets_threshold(&self) -> bool {
        self.node_count >= STRINGIFY_STATIC_NODE_COUNT
            || self.element_with_binding_count >= STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT
    }

    pub(crate) fn render_static_call(&self) -> String {
        format!(
            "_createStaticVNode({}, {})",
            self.html.to_js_expression(),
            self.dom_nodes
        )
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StaticHtmlBuffer {
    pub(crate) parts: Vec<StaticHtmlPart>,
}

#[derive(Clone, Debug)]
pub(crate) enum StaticHtmlPart {
    Text(String),
    Expression(String),
}

impl StaticHtmlBuffer {
    pub(crate) fn from_text(value: impl Into<String>) -> Self {
        let mut buffer = Self::default();
        buffer.push_text(value);
        buffer
    }

    pub(crate) fn push_text(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value.is_empty() {
            return;
        }
        match self.parts.last_mut() {
            Some(StaticHtmlPart::Text(existing)) => existing.push_str(&value),
            _ => self.parts.push(StaticHtmlPart::Text(value)),
        }
    }

    pub(crate) fn push_expression(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value.trim().is_empty() {
            return;
        }
        self.parts.push(StaticHtmlPart::Expression(value));
    }

    pub(crate) fn append(&mut self, other: Self) {
        for part in other.parts {
            match part {
                StaticHtmlPart::Text(value) => self.push_text(value),
                StaticHtmlPart::Expression(value) => self.push_expression(value),
            }
        }
    }

    pub(crate) fn to_js_expression(&self) -> String {
        let parts = self
            .parts
            .iter()
            .filter_map(|part| match part {
                StaticHtmlPart::Text(value) if !value.is_empty() => Some(quote_string(value)),
                StaticHtmlPart::Text(_) => None,
                StaticHtmlPart::Expression(value) => Some(value.clone()),
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            quote_string("")
        } else {
            parts.join(" + ")
        }
    }
}

pub(crate) fn render_static_vnode_cache(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let analysis = analyze_static_html_chunk(ast, children, options, scope)?;
    if analysis.meets_threshold() {
        Some(analysis.render_static_call())
    } else {
        None
    }
}

pub(crate) fn render_root_static_vnode_cache(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if visible_child_ids(ast, children).len() < 2 {
        return None;
    }
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    let analysis = analyze_static_html_chunk(ast, &child_nodes, options, scope)?;
    (analysis.dom_nodes > 1 || analysis.meets_threshold()).then(|| analysis.render_static_call())
}

pub(crate) fn render_static_vnode_chunked_children(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    regular_mode: NodeRenderMode,
    memo_index: &mut MemoIndex,
) -> Option<Vec<String>> {
    let chunks = static_vnode_chunks(ast, children, options, scope);
    if chunks.is_empty() {
        return None;
    }

    let mut rendered = Vec::new();
    let mut cursor = 0usize;
    for chunk in chunks {
        render_static_vnode_regular_segment(
            ast,
            children,
            cursor,
            chunk.start,
            options,
            scope,
            regular_mode,
            memo_index,
            &mut rendered,
        );
        rendered.push(chunk.call);
        cursor = chunk.end;
    }
    render_static_vnode_regular_segment(
        ast,
        children,
        cursor,
        children.len(),
        options,
        scope,
        regular_mode,
        memo_index,
        &mut rendered,
    );
    Some(rendered)
}

pub(crate) fn collect_static_hoists(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> StaticHoists {
    let mut hoists = StaticHoists::default();
    if !options.hoist_static {
        return hoists;
    }
    let stringified_nodes = collect_stringified_static_node_ids(ast, options);
    let do_not_hoist_root = ast
        .root_node()
        .and_then(|root| vue3_single_static_root_child(&root.children, ast));
    collect_static_hoists_for_node(
        ast,
        ast.root,
        options,
        do_not_hoist_root,
        &stringified_nodes,
        &mut hoists,
    );
    hoists
}

pub(crate) fn collect_static_hoists_for_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    do_not_hoist_root: Option<vuec_ast::NodeId>,
    stringified_nodes: &BTreeSet<vuec_ast::NodeId>,
    hoists: &mut StaticHoists,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        collect_static_asset_binding_hoists(node_id, element, hoists);
        if !stringified_nodes.contains(&node_id)
            && static_props_should_hoist_element(ast, node, element, options, do_not_hoist_root)
        {
            hoists.push_props_object(node_id);
        }
    }
    if stringified_nodes.contains(&node_id) {
        for child_id in &node.children {
            collect_static_asset_binding_hoists_for_subtree(ast, *child_id, hoists);
        }
        return;
    }
    for child_id in &node.children {
        collect_static_hoists_for_node(
            ast,
            *child_id,
            options,
            do_not_hoist_root,
            stringified_nodes,
            hoists,
        );
    }
}

pub(crate) fn collect_static_asset_binding_hoists_for_subtree(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    hoists: &mut StaticHoists,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        collect_static_asset_binding_hoists(node_id, element, hoists);
    }
    for child_id in &node.children {
        collect_static_asset_binding_hoists_for_subtree(ast, *child_id, hoists);
    }
}

pub(crate) fn collect_stringified_static_node_ids(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> BTreeSet<vuec_ast::NodeId> {
    let mut ids = BTreeSet::new();
    if !options.stringify_static {
        return ids;
    }
    let scope = RenderScope::default();
    if let Some(root) = ast.root_node() {
        let root_children = root
            .children
            .iter()
            .filter_map(|child_id| ast.node(*child_id))
            .collect::<Vec<_>>();
        if visible_child_ids(ast, &root.children).len() >= 2
            && analyze_static_html_chunk(ast, &root_children, options, &scope)
                .is_some_and(|analysis| analysis.dom_nodes > 1 || analysis.meets_threshold())
        {
            for child in &root_children {
                collect_static_subtree_ids(ast, child.id, &mut ids);
            }
        }
    }
    collect_stringified_static_node_ids_for_parent(ast, ast.root, options, &scope, &mut ids);
    ids
}

pub(crate) fn collect_stringified_static_node_ids_for_parent(
    ast: &Vue3Ast,
    parent_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    ids: &mut BTreeSet<vuec_ast::NodeId>,
) {
    let Some(parent) = ast.node(parent_id) else {
        return;
    };
    let children = parent
        .children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    for chunk in static_vnode_chunks(ast, &children, options, scope) {
        for child in &children[chunk.start..chunk.end] {
            collect_static_subtree_ids(ast, child.id, ids);
        }
    }
    for child_id in &parent.children {
        if !ids.contains(child_id) {
            collect_stringified_static_node_ids_for_parent(ast, *child_id, options, scope, ids);
        }
    }
}

pub(crate) fn collect_static_subtree_ids(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    ids: &mut BTreeSet<vuec_ast::NodeId>,
) {
    if !ids.insert(node_id) {
        return;
    }
    if let Some(node) = ast.node(node_id) {
        for child_id in &node.children {
            collect_static_subtree_ids(ast, *child_id, ids);
        }
    }
}

pub(crate) fn collect_static_asset_binding_hoists(
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    hoists: &mut StaticHoists,
) {
    for (prop_index, prop) in element.props.iter().enumerate() {
        let Vue3Prop::Directive(dir) = prop else {
            continue;
        };
        let Some(expression) = static_asset_binding_expression_to_hoist(dir) else {
            continue;
        };
        let key = render_static_binding_prop_key(dir);
        hoists.push_binding(node_id, prop_index, expression, key != "srcset");
    }
}

pub(crate) fn static_asset_binding_expression_to_hoist(dir: &Vue3Directive) -> Option<String> {
    if dir.name != "bind" || dir.is_dynamic_arg || !dir.modifiers.is_empty() {
        return None;
    }
    let key = render_static_binding_prop_key(dir);
    let expression = dir.exp.as_ref()?.source_string();
    let expression = expression.trim();
    if !expression_is_generated_asset_import(expression) {
        return None;
    }
    (key == "srcset" || generated_asset_import_expression_has_literal(expression))
        .then(|| expression.to_string())
}

pub(crate) fn static_props_should_hoist_element(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    element: &Vue3Element,
    _options: &Vue3CompilerOptions,
    do_not_hoist_root: Option<vuec_ast::NodeId>,
) -> bool {
    if element.props.is_empty()
        || element.tag_type != Vue3ElementType::Element
        || directive_by_name(element, "if").is_some()
        || directive_by_name(element, "else").is_some()
        || directive_by_name(element, "else-if").is_some()
        || directive_by_name(element, "for").is_some()
        || !element
            .props
            .iter()
            .all(vue3_prop_is_static_cacheable_for_hoist)
    {
        return false;
    }
    do_not_hoist_root == Some(node.id) || !is_static_element_tree_for_cache(ast, node)
}

pub(crate) fn static_hoist_declarations(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
    hoists: &StaticHoists,
) -> Vec<String> {
    let scope = RenderScope::default().with_static_hoists(hoists.clone());
    hoists
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(declaration_index, declaration)| {
            let index = declaration_index + 1;
            match declaration {
                StaticHoistDeclaration::BindingExpression { expression } => {
                    Some(format!("const _hoisted_{index} = {expression}"))
                }
                StaticHoistDeclaration::PropsObject { node_id } => {
                    let node = ast.node(*node_id)?;
                    let Vue3AstKind::Element(element) = &node.kind else {
                        return None;
                    };
                    let props =
                        render_static_props_hoist_object(*node_id, element, options, &scope)?;
                    Some(format!("const _hoisted_{index} = {props}"))
                }
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct StaticVNodeChunk {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) call: String,
}

pub(crate) fn static_vnode_chunks(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Vec<StaticVNodeChunk> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut chunk_analysis = None::<StaticHtmlAnalysis>;
    let mut blocked_by_comment = false;

    for (index, child) in children.iter().enumerate() {
        if matches!(child.kind, Vue3AstKind::Comment(_)) {
            blocked_by_comment = true;
        }
        if let Some(analysis) = analyze_static_html_chunk(ast, &[*child], options, scope) {
            if chunk_analysis.is_none() {
                start = index;
            }
            match chunk_analysis.as_mut() {
                Some(existing) => existing.append(analysis),
                None => chunk_analysis = Some(analysis),
            }
            continue;
        }
        push_static_vnode_chunk(
            &mut chunks,
            start,
            index,
            &mut chunk_analysis,
            blocked_by_comment,
        );
        blocked_by_comment = false;
    }
    push_static_vnode_chunk(
        &mut chunks,
        start,
        children.len(),
        &mut chunk_analysis,
        blocked_by_comment,
    );
    chunks
}

pub(crate) fn push_static_vnode_chunk(
    chunks: &mut Vec<StaticVNodeChunk>,
    start: usize,
    end: usize,
    chunk_analysis: &mut Option<StaticHtmlAnalysis>,
    blocked_by_comment: bool,
) {
    let Some(analysis) = chunk_analysis.take() else {
        return;
    };
    if analysis.meets_threshold() && !blocked_by_comment {
        chunks.push(StaticVNodeChunk {
            start,
            end,
            call: analysis.render_static_call(),
        });
    }
}

pub(crate) fn render_static_vnode_regular_segment(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    mode: NodeRenderMode,
    memo_index: &mut MemoIndex,
    rendered: &mut Vec<String>,
) {
    if start >= end {
        return;
    }
    let ids = children[start..end]
        .iter()
        .map(|child| child.id)
        .collect::<Vec<_>>();
    rendered.extend(render_child_sequence(
        ast, &ids, options, mode, scope, memo_index,
    ));
}

pub(crate) fn analyze_static_html_chunk(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<StaticHtmlAnalysis> {
    if children.is_empty() {
        return None;
    }
    let mut analysis = StaticHtmlAnalysis {
        html: StaticHtmlBuffer::default(),
        dom_nodes: children.len(),
        node_count: 0,
        element_with_binding_count: 0,
    };
    for child in children {
        analysis
            .html
            .append(static_html_for_node(ast, child, options, scope)?);
    }
    accumulate_static_html_analysis(ast, children, &mut analysis)?;
    Some(analysis)
}

pub(crate) fn static_html_for_node(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<StaticHtmlBuffer> {
    match &node.kind {
        Vue3AstKind::Element(element) => {
            static_html_for_element(ast, node, element, options, scope)
        }
        Vue3AstKind::Text(text) => Some(StaticHtmlBuffer::from_text(escape_static_html_text(
            &text.value,
        ))),
        Vue3AstKind::Interpolation(interpolation) => {
            let value = static_const_eval_source(&interpolation.expression.source_string())?;
            Some(StaticHtmlBuffer::from_text(escape_static_html_text(
                &value.to_display_string()?,
            )))
        }
        _ => None,
    }
}

pub(crate) fn static_html_for_element(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<StaticHtmlBuffer> {
    if element.tag == "slot"
        || element.tag_type != Vue3ElementType::Element
        || (element.ns == vuec_ast::HtmlNamespace::Html
            && static_html_non_stringifiable_tag(&element.tag))
        || (element.ns == vuec_ast::HtmlNamespace::Html
            && static_html_is_void_tag(&element.tag)
            && !node.children.is_empty())
        || static_html_has_invalid_inner_html_placement(ast, node, element)
        || directive_by_name(element, "once").is_some()
    {
        return None;
    }

    let mut html = StaticHtmlBuffer::default();
    html.push_text("<");
    html.push_text(element.tag.as_str());
    let mut inner_html = None;
    for (prop_index, prop) in element.props.iter().enumerate() {
        match prop {
            Vue3Prop::Attribute(attr) => {
                if !static_html_attr_is_stringifiable(&attr.name, element.ns) {
                    return None;
                }
                html.push_text(" ");
                html.push_text(attr.name.as_str());
                if let Some(value) = &attr.value {
                    html.push_text("=\"");
                    html.push_text(escape_static_html_attr(value));
                    html.push_text("\"");
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "html" => {
                let source = dir.exp.as_ref()?.source_string();
                let value = static_const_eval_source(&source)?;
                inner_html = Some(decode_static_html_entities(&value.to_display_string()?));
            }
            Vue3Prop::Directive(dir) if dir.name == "text" => {
                let source = dir.exp.as_ref()?.source_string();
                let value = static_const_eval_source(&source)?;
                inner_html = Some(escape_static_html_text(&value.to_display_string()?));
            }
            Vue3Prop::Directive(dir) => {
                let Some(rendered) = static_html_directive_attr(
                    &element.tag,
                    element.ns,
                    node.id,
                    prop_index,
                    dir,
                    scope,
                )?
                else {
                    continue;
                };
                html.push_text(" ");
                html.push_text(rendered.name.as_str());
                html.push_text("=\"");
                html.append(rendered.value);
                html.push_text("\"");
            }
        }
    }
    if let Some(scope_id) = options
        .scope_id
        .as_deref()
        .filter(|scope_id| !scope_id.is_empty())
    {
        html.push_text(" ");
        html.push_text(scope_id);
    }
    html.push_text(">");

    if element.ns != vuec_ast::HtmlNamespace::Html || !static_html_is_void_tag(&element.tag) {
        if let Some(inner_html) = inner_html.filter(|value| !value.is_empty()) {
            html.push_text(inner_html);
        } else {
            for child_id in &node.children {
                let child = ast.node(*child_id)?;
                html.append(static_html_for_node(ast, child, options, scope)?);
            }
        }
        html.push_text("</");
        html.push_text(element.tag.as_str());
        html.push_text(">");
    }

    Some(html)
}

#[derive(Clone, Debug)]
pub(crate) struct StaticHtmlAttr {
    pub(crate) name: String,
    pub(crate) value: StaticHtmlBuffer,
}

pub(crate) fn static_html_directive_attr(
    tag: &str,
    ns: vuec_ast::HtmlNamespace,
    node_id: vuec_ast::NodeId,
    prop_index: usize,
    dir: &Vue3Directive,
    scope: &RenderScope,
) -> Option<Option<StaticHtmlAttr>> {
    match dir.name.as_str() {
        "bind" => static_html_bind_attr(tag, ns, node_id, prop_index, dir, scope),
        "html" | "text" => None,
        _ => None,
    }
}

pub(crate) fn static_html_bind_attr(
    tag: &str,
    ns: vuec_ast::HtmlNamespace,
    node_id: vuec_ast::NodeId,
    prop_index: usize,
    dir: &Vue3Directive,
    scope: &RenderScope,
) -> Option<Option<StaticHtmlAttr>> {
    if dir.is_dynamic_arg || !dir.modifiers.is_empty() {
        return None;
    }
    let name = dir.arg.as_ref()?.source_string();
    if !static_html_attr_is_stringifiable(&name, ns) {
        return None;
    }
    if ns == vuec_ast::HtmlNamespace::Html && tag == "option" && name == "value" {
        return None;
    }
    let source = dir.exp.as_ref()?.source_string();
    if is_asset_import_binding(dir) {
        if let Some(index) = scope.static_hoists.binding_index(node_id, prop_index) {
            let mut value = StaticHtmlBuffer::default();
            value.push_expression(format!("_hoisted_{index}"));
            return Some(Some(StaticHtmlAttr { name, value }));
        }
        return Some(Some(StaticHtmlAttr {
            name,
            value: static_html_asset_import_expression(&source)?,
        }));
    }
    let value = static_const_eval_source(&source)?;
    if matches!(value, StaticConstValue::Null) {
        return Some(None);
    }
    if static_html_is_boolean_attr(&name) && matches!(value, StaticConstValue::Bool(false)) {
        return Some(None);
    }
    let value = if name == "class" {
        static_const_normalize_class(&value)?
    } else if name == "style" {
        static_const_stringify_style(&value)?
    } else {
        value.to_display_string()?
    };
    Some(Some(StaticHtmlAttr {
        name,
        value: StaticHtmlBuffer::from_text(escape_static_html_attr(&value)),
    }))
}

pub(crate) fn accumulate_static_html_analysis(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    analysis: &mut StaticHtmlAnalysis,
) -> Option<()> {
    for child in children {
        match &child.kind {
            Vue3AstKind::Element(element) => {
                analysis.node_count += 1;
                if !element.props.is_empty() {
                    analysis.element_with_binding_count += 1;
                }
                let descendants = child
                    .children
                    .iter()
                    .filter_map(|child_id| ast.node(*child_id))
                    .collect::<Vec<_>>();
                accumulate_static_html_analysis(ast, &descendants, analysis)?;
            }
            Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_) => {
                analysis.node_count += 1;
            }
            _ => return None,
        }
    }
    Some(())
}

pub(crate) const STATIC_HTML_KNOWN_HTML_ATTRS: &str = "accept,accept-charset,accesskey,action,align,allow,alt,async,autocapitalize,autocomplete,autofocus,autoplay,background,bgcolor,border,buffered,capture,challenge,charset,checked,cite,class,code,codebase,color,cols,colspan,content,contenteditable,contextmenu,controls,coords,crossorigin,csp,data,datetime,decoding,default,defer,dir,dirname,disabled,download,draggable,dropzone,enctype,enterkeyhint,for,form,formaction,formenctype,formmethod,formnovalidate,formtarget,headers,height,hidden,high,href,hreflang,http-equiv,icon,id,importance,inert,integrity,ismap,itemprop,keytype,kind,label,lang,language,loading,list,loop,low,manifest,max,maxlength,minlength,media,min,multiple,muted,name,novalidate,open,optimum,pattern,ping,placeholder,poster,preload,radiogroup,readonly,referrerpolicy,rel,required,reversed,rows,rowspan,sandbox,scope,scoped,selected,shape,size,sizes,slot,span,spellcheck,src,srcdoc,srclang,srcset,start,step,style,summary,tabindex,target,title,translate,type,usemap,value,width,wrap";

pub(crate) const STATIC_HTML_KNOWN_SVG_ATTRS: &str = "xmlns,accent-height,accumulate,additive,alignment-baseline,alphabetic,amplitude,arabic-form,ascent,attributeName,attributeType,azimuth,baseFrequency,baseline-shift,baseProfile,bbox,begin,bias,by,calcMode,cap-height,class,clip,clipPathUnits,clip-path,clip-rule,color,color-interpolation,color-interpolation-filters,color-profile,color-rendering,contentScriptType,contentStyleType,crossorigin,cursor,cx,cy,d,decelerate,descent,diffuseConstant,direction,display,divisor,dominant-baseline,dur,dx,dy,edgeMode,elevation,enable-background,end,exponent,fill,fill-opacity,fill-rule,filter,filterRes,filterUnits,flood-color,flood-opacity,font-family,font-size,font-size-adjust,font-stretch,font-style,font-variant,font-weight,format,from,fr,fx,fy,g1,g2,glyph-name,glyph-orientation-horizontal,glyph-orientation-vertical,glyphRef,gradientTransform,gradientUnits,hanging,height,href,hreflang,horiz-adv-x,horiz-origin-x,id,ideographic,image-rendering,in,in2,intercept,k,k1,k2,k3,k4,kernelMatrix,kernelUnitLength,kerning,keyPoints,keySplines,keyTimes,lang,lengthAdjust,letter-spacing,lighting-color,limitingConeAngle,local,marker-end,marker-mid,marker-start,markerHeight,markerUnits,markerWidth,mask,maskContentUnits,maskUnits,mathematical,max,media,method,min,mode,name,numOctaves,offset,opacity,operator,order,orient,orientation,origin,overflow,overline-position,overline-thickness,panose-1,paint-order,path,pathLength,patternContentUnits,patternTransform,patternUnits,ping,pointer-events,points,pointsAtX,pointsAtY,pointsAtZ,preserveAlpha,preserveAspectRatio,primitiveUnits,r,radius,referrerPolicy,refX,refY,rel,rendering-intent,repeatCount,repeatDur,requiredExtensions,requiredFeatures,restart,result,rotate,rx,ry,scale,seed,shape-rendering,slope,spacing,specularConstant,specularExponent,speed,spreadMethod,startOffset,stdDeviation,stemh,stemv,stitchTiles,stop-color,stop-opacity,strikethrough-position,strikethrough-thickness,string,stroke,stroke-dasharray,stroke-dashoffset,stroke-linecap,stroke-linejoin,stroke-miterlimit,stroke-opacity,stroke-width,style,surfaceScale,systemLanguage,tabindex,tableValues,target,targetX,targetY,text-anchor,text-decoration,text-rendering,textLength,to,transform,transform-origin,type,u1,u2,underline-position,underline-thickness,unicode,unicode-bidi,unicode-range,units-per-em,v-alphabetic,v-hanging,v-ideographic,v-mathematical,values,vector-effect,version,vert-adv-y,vert-origin-x,vert-origin-y,viewBox,viewTarget,visibility,width,widths,word-spacing,writing-mode,x,x-height,x1,x2,xChannelSelector,xlink:actuate,xlink:arcrole,xlink:href,xlink:role,xlink:show,xlink:title,xlink:type,xmlns:xlink,xml:base,xml:lang,xml:space,y,y1,y2,yChannelSelector,z,zoomAndPan";

pub(crate) const STATIC_HTML_KNOWN_MATHML_ATTRS: &str = "accent,accentunder,actiontype,align,alignmentscope,altimg,altimg-height,altimg-valign,altimg-width,alttext,bevelled,close,columnsalign,columnlines,columnspan,denomalign,depth,dir,display,displaystyle,encoding,equalcolumns,equalrows,fence,fontstyle,fontweight,form,frame,framespacing,groupalign,height,href,id,indentalign,indentalignfirst,indentalignlast,indentshift,indentshiftfirst,indentshiftlast,indextype,justify,largetop,largeop,lquote,lspace,mathbackground,mathcolor,mathsize,mathvariant,maxsize,minlabelspacing,mode,other,overflow,position,rowalign,rowlines,rowspan,rquote,rspace,scriptlevel,scriptminsize,scriptsizemultiplier,selection,separator,separators,shift,side,src,stackalign,stretchy,subscriptshift,superscriptshift,symmetric,voffset,width,widths,xlink:href,xlink:show,xlink:type,xmlns";

pub(crate) fn static_html_attr_is_stringifiable(name: &str, ns: vuec_ast::HtmlNamespace) -> bool {
    name.starts_with("data-")
        || name.starts_with("aria-")
        || match ns {
            vuec_ast::HtmlNamespace::Html => {
                static_html_known_attr_contains(STATIC_HTML_KNOWN_HTML_ATTRS, name)
            }
            vuec_ast::HtmlNamespace::Svg => {
                static_html_known_attr_contains(STATIC_HTML_KNOWN_SVG_ATTRS, name)
            }
            vuec_ast::HtmlNamespace::MathMl => {
                static_html_known_attr_contains(STATIC_HTML_KNOWN_MATHML_ATTRS, name)
            }
        }
}

pub(crate) fn static_html_known_attr_contains(attrs: &str, name: &str) -> bool {
    attrs.split(',').any(|attr| attr == name)
}

pub(crate) fn static_html_is_boolean_attr(name: &str) -> bool {
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
            | "selected"
    )
}

pub(crate) fn static_html_non_stringifiable_tag(tag: &str) -> bool {
    matches!(
        tag,
        "caption" | "thead" | "tr" | "th" | "tbody" | "td" | "tfoot" | "colgroup" | "col"
    )
}

pub(crate) fn static_html_has_invalid_inner_html_placement(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    element: &Vue3Element,
) -> bool {
    element.ns == vuec_ast::HtmlNamespace::Html
        && element.tag.eq_ignore_ascii_case("p")
        && node
            .children
            .iter()
            .any(|child_id| static_html_contains_invalid_p_descendant(ast, *child_id))
}

pub(crate) fn static_html_contains_invalid_p_descendant(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
) -> bool {
    let Some(node) = ast.node(node_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    if element.ns != vuec_ast::HtmlNamespace::Html {
        return false;
    }
    static_html_is_invalid_p_child_tag(&element.tag)
        || node
            .children
            .iter()
            .any(|child_id| static_html_contains_invalid_p_descendant(ast, *child_id))
}

pub(crate) fn static_html_is_invalid_p_child_tag(tag: &str) -> bool {
    static_html_tag_list_contains(STATIC_HTML_INVALID_P_CHILD_TAGS, tag)
}

pub(crate) const STATIC_HTML_INVALID_P_CHILD_TAGS: &str = "address,article,aside,blockquote,center,details,dialog,dir,div,dl,fieldset,figure,footer,form,h1,h2,h3,h4,h5,h6,header,hgroup,hr,li,main,nav,menu,ol,p,pre,section,table,ul";

pub(crate) fn static_html_tag_list_contains(tags: &str, tag: &str) -> bool {
    tags.split(',')
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

pub(crate) fn static_html_is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub(crate) fn escape_static_html_text(value: &str) -> String {
    escape_static_html(value, false)
}

pub(crate) fn escape_static_html_attr(value: &str) -> String {
    escape_static_html(value, true)
}

pub(crate) fn decode_static_html_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

pub(crate) fn escape_static_html(value: &str, attr: bool) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if attr => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StaticConstValue {
    String(String),
    Number(String),
    Bool(bool),
    Null,
    Array(Vec<StaticConstValue>),
    Object(Vec<(String, StaticConstValue)>),
}

impl StaticConstValue {
    pub(crate) fn to_display_string(&self) -> Option<String> {
        match self {
            Self::String(value) | Self::Number(value) => Some(value.clone()),
            Self::Bool(true) => Some("true".into()),
            Self::Bool(false) => Some("false".into()),
            Self::Null => Some(String::new()),
            Self::Array(_) | Self::Object(_) => None,
        }
    }

    pub(crate) fn to_js_string(&self) -> Option<String> {
        match self {
            Self::String(value) | Self::Number(value) => Some(value.clone()),
            Self::Bool(true) => Some("true".into()),
            Self::Bool(false) => Some("false".into()),
            Self::Null => Some("null".into()),
            Self::Array(_) | Self::Object(_) => None,
        }
    }

    pub(crate) fn truthy(&self) -> bool {
        match self {
            Self::String(value) => !value.is_empty(),
            Self::Number(value) => !matches!(value.as_str(), "0" | "-0" | "NaN"),
            Self::Bool(value) => *value,
            Self::Null => false,
            Self::Array(_) | Self::Object(_) => true,
        }
    }
}

pub(crate) fn static_const_eval_source(source: &str) -> Option<StaticConstValue> {
    let store = JsAstStore::new();
    let expression = store
        .parse_expression(source.trim(), oxc_span::SourceType::ts())
        .ok()?;
    static_const_eval_expression(&expression)
}

pub(crate) fn static_const_eval_expression(
    expression: &Expression<'_>,
) -> Option<StaticConstValue> {
    match expression {
        Expression::StringLiteral(literal) => {
            Some(StaticConstValue::String(literal.value.as_str().to_string()))
        }
        Expression::NumericLiteral(literal) => Some(StaticConstValue::Number(
            static_const_number_string(literal.value),
        )),
        Expression::BooleanLiteral(literal) => Some(StaticConstValue::Bool(literal.value)),
        Expression::NullLiteral(_) => Some(StaticConstValue::Null),
        Expression::TemplateLiteral(literal) => {
            if !literal.expressions.is_empty() || literal.quasis.len() != 1 {
                return None;
            }
            let cooked = literal.quasis.first()?.value.cooked.as_ref()?;
            Some(StaticConstValue::String(cooked.as_str().to_string()))
        }
        Expression::ParenthesizedExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::UnaryExpression(expression) => static_const_eval_unary(expression),
        Expression::BinaryExpression(expression) => static_const_eval_binary(expression),
        Expression::ArrayExpression(expression) => {
            let mut values = Vec::new();
            for element in &expression.elements {
                values.push(static_const_eval_array_element(element)?);
            }
            Some(StaticConstValue::Array(values))
        }
        Expression::ObjectExpression(expression) => {
            let mut values = Vec::new();
            for property in &expression.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                if property.kind != PropertyKind::Init
                    || property.method
                    || property.shorthand
                    || property.computed
                {
                    return None;
                }
                let key = static_const_property_key(&property.key)?;
                let value = static_const_eval_expression(&property.value)?;
                values.push((key, value));
            }
            Some(StaticConstValue::Object(values))
        }
        _ => None,
    }
}

pub(crate) fn static_const_eval_array_element(
    element: &ArrayExpressionElement<'_>,
) -> Option<StaticConstValue> {
    if element.is_elision() || element.is_spread() {
        return None;
    }
    static_const_eval_expression(element.as_expression()?)
}

pub(crate) fn static_const_eval_unary(
    expression: &oxc_ast::ast::UnaryExpression<'_>,
) -> Option<StaticConstValue> {
    let value = static_const_eval_expression(&expression.argument)?;
    match expression.operator {
        UnaryOperator::LogicalNot => Some(StaticConstValue::Bool(!value.truthy())),
        UnaryOperator::UnaryPlus => Some(StaticConstValue::Number(static_const_number_string(
            static_const_to_number(&value)?,
        ))),
        UnaryOperator::UnaryNegation => Some(StaticConstValue::Number(static_const_number_string(
            -static_const_to_number(&value)?,
        ))),
        _ => None,
    }
}

pub(crate) fn static_const_eval_binary(
    expression: &oxc_ast::ast::BinaryExpression<'_>,
) -> Option<StaticConstValue> {
    if expression.operator != BinaryOperator::Addition {
        return None;
    }
    let left = static_const_eval_expression(&expression.left)?;
    let right = static_const_eval_expression(&expression.right)?;
    if matches!(left, StaticConstValue::String(_)) || matches!(right, StaticConstValue::String(_)) {
        Some(StaticConstValue::String(format!(
            "{}{}",
            left.to_js_string()?,
            right.to_js_string()?
        )))
    } else {
        Some(StaticConstValue::Number(static_const_number_string(
            static_const_to_number(&left)? + static_const_to_number(&right)?,
        )))
    }
}

pub(crate) fn static_const_to_number(value: &StaticConstValue) -> Option<f64> {
    match value {
        StaticConstValue::String(value) if value.trim().is_empty() => Some(0.0),
        StaticConstValue::String(value) => value.trim().parse::<f64>().ok(),
        StaticConstValue::Number(value) => value.parse::<f64>().ok(),
        StaticConstValue::Bool(true) => Some(1.0),
        StaticConstValue::Bool(false) | StaticConstValue::Null => Some(0.0),
        StaticConstValue::Array(_) | StaticConstValue::Object(_) => None,
    }
}

pub(crate) fn static_const_property_key(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str().to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.as_str().to_string()),
        PropertyKey::NumericLiteral(literal) => Some(static_const_number_string(literal.value)),
        _ => None,
    }
}

pub(crate) fn static_const_number_string(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-Infinity".into()
        } else {
            "Infinity".into()
        }
    } else if value == 0.0 {
        "0".into()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

pub(crate) fn static_const_normalize_class(value: &StaticConstValue) -> Option<String> {
    match value {
        StaticConstValue::String(value) => Some(value.clone()),
        StaticConstValue::Array(items) => {
            let mut classes = Vec::new();
            for item in items {
                let normalized = static_const_normalize_class(item)?;
                if !normalized.is_empty() {
                    classes.push(normalized);
                }
            }
            Some(classes.join(" "))
        }
        StaticConstValue::Object(properties) => Some(
            properties
                .iter()
                .filter_map(|(key, value)| value.truthy().then(|| key.clone()))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        StaticConstValue::Bool(_) | StaticConstValue::Number(_) | StaticConstValue::Null => {
            Some(String::new())
        }
    }
}

pub(crate) fn static_const_stringify_style(value: &StaticConstValue) -> Option<String> {
    match value {
        StaticConstValue::String(value) => {
            let style = vue3_parse_static_style(value);
            Some(static_const_stringify_style_entries(style))
        }
        StaticConstValue::Object(properties) => Some(static_const_stringify_style_entries(
            properties
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .to_display_string()
                        .filter(|_| !matches!(value, StaticConstValue::Null))
                        .map(|value| (hyphenate_style_property(key), value))
                })
                .filter(|(_, value)| !value.is_empty())
                .collect(),
        )),
        StaticConstValue::Array(items) => {
            let mut entries = Vec::new();
            for item in items {
                match item {
                    StaticConstValue::String(value) => {
                        entries.extend(vue3_parse_static_style(value));
                    }
                    StaticConstValue::Object(properties) => {
                        entries.extend(properties.iter().filter_map(|(key, value)| {
                            value
                                .to_display_string()
                                .filter(|_| !matches!(value, StaticConstValue::Null))
                                .map(|value| (hyphenate_style_property(key), value))
                        }));
                    }
                    _ => return None,
                }
            }
            Some(static_const_stringify_style_entries(entries))
        }
        StaticConstValue::Bool(_) | StaticConstValue::Number(_) | StaticConstValue::Null => None,
    }
}

pub(crate) fn static_const_stringify_style_entries(entries: Vec<(String, String)>) -> String {
    entries
        .into_iter()
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .map(|(key, value)| format!("{key}:{value};"))
        .collect::<Vec<_>>()
        .join("")
}

pub(crate) fn hyphenate_style_property(value: &str) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                output.push('-');
            }
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn vue3_single_static_root_child(children: &[NodeId], ast: &Vue3Ast) -> Option<NodeId> {
    let mut element = None;
    for child_id in children {
        let Some(child) = ast.node(*child_id) else {
            continue;
        };
        if matches!(child.kind, Vue3AstKind::Comment(_)) {
            continue;
        }
        let Vue3AstKind::Element(element_kind) = &child.kind else {
            return None;
        };
        if element_kind.tag_type == Vue3ElementType::SlotOutlet {
            return None;
        }
        if element.replace(*child_id).is_some() {
            return None;
        }
    }
    element
}

pub(crate) fn vue3_dom_mir_can_hoist_static_node(ast: &Vue3Ast, node_id: NodeId) -> bool {
    let Some(node) = ast.node(node_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    if element.tag == "slot"
        || element.tag_type != Vue3ElementType::Element
        || !element
            .props
            .iter()
            .all(vue3_prop_is_vnode_cacheable_static)
    {
        return false;
    }
    node.children.iter().all(|child_id| {
        let Some(child) = ast.node(*child_id) else {
            return false;
        };
        match &child.kind {
            Vue3AstKind::Text(_) | Vue3AstKind::Comment(_) => true,
            Vue3AstKind::Element(_) => vue3_dom_mir_can_hoist_static_node(ast, *child_id),
            _ => false,
        }
    })
}

pub(crate) fn child_sequence_is_direct_dynamic_text(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
) -> bool {
    let visible = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    !visible.is_empty()
        && visible.iter().all(|child| {
            matches!(
                child.kind,
                Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
            )
        })
        && !children_literal_const_only(ast, children, options)
        && visible
            .iter()
            .any(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
}

pub(crate) fn child_sequence_needs_text_vnode(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
) -> bool {
    let visible = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    visible.len() > 1 && !visible.iter().all(|child| is_text_like(child))
}

pub(crate) fn children_literal_const_only(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
) -> bool {
    let mut has_interpolation = false;
    for child_id in children {
        let Some(child) = ast.node(*child_id) else {
            continue;
        };
        match &child.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                has_interpolation = true;
                let expression = interpolation.expression.source_string();
                if options
                    .binding_metadata
                    .get(expression.trim())
                    .map(String::as_str)
                    != Some("literal-const")
                {
                    return false;
                }
            }
            Vue3AstKind::Text(text) if text.value.trim().is_empty() => {}
            Vue3AstKind::Comment(_) => {}
            _ => return false,
        }
    }
    has_interpolation
}

pub(crate) fn render_patch_flag_kind(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
) -> Option<i32> {
    let children = ast
        .node(node_id)
        .map(|node| node.children.as_slice())
        .unwrap_or(&[]);
    if mode == NodeRenderMode::Cached {
        Some(-1)
    } else if has_dynamic_arg_binding(element) {
        Some(16)
    } else {
        let mut flag = 0;
        if element.tag_type == Vue3ElementType::Element && has_class_binding(element) {
            flag |= 2;
        }
        if element.tag_type == Vue3ElementType::Element && has_style_binding(element) {
            flag |= 4;
        }
        if has_dynamic_non_key_props(element, options, scope)
            && !(mode == NodeRenderMode::Cached && static_cached_bindings_are_constant(element))
        {
            flag |= 8;
        }
        if !once_children_mode(mode)
            && element.tag != "template"
            && child_sequence_is_direct_dynamic_text(ast, children, options)
        {
            flag |= 1;
        }
        if flag == 0
            && (has_vnode_hook(element)
                || has_runtime_directive(element)
                || has_native_v_model(element))
        {
            flag |= 512;
        }
        (flag != 0).then_some(flag)
    }
}

pub(crate) fn render_patch_flag_text(flag: Option<i32>) -> String {
    match flag {
        Some(flag) => format!(", {}", public_patch_flag_text(flag)),
        None => String::new(),
    }
}

/// Computes the public codegen patch flag for a Vue 3 element node.
pub fn vue3_element_codegen_patch_flag(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    is_root: bool,
) -> Option<i32> {
    let Some(node) = ast.node(node_id) else {
        return None;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return None;
    };
    let mode = if is_root {
        NodeRenderMode::Root
    } else {
        NodeRenderMode::Child
    };
    let scope = RenderScope::default();
    render_patch_flag_kind(ast, node_id, element, options, mode, &scope)
}

pub(crate) fn render_props(
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    render_props_for_target(
        Some(node_id),
        element,
        options,
        scope,
        branch_key,
        memo_index,
        ExactPropsTarget::VNode,
    )
}

pub(crate) fn render_slot_outlet_props(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    render_props_for_target(
        None,
        element,
        options,
        scope,
        None,
        memo_index,
        ExactPropsTarget::SlotOutlet,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactPropsTarget {
    VNode,
    SlotOutlet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactPropsArgKind {
    Object,
    ObjectBinding,
    ObjectListeners,
    DynamicEvent,
}

#[derive(Clone, Debug)]
pub(crate) struct ExactPropsArg {
    pub(crate) kind: ExactPropsArgKind,
    pub(crate) code: String,
}

pub(crate) fn render_props_for_target(
    node_id: Option<vuec_ast::NodeId>,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
    target: ExactPropsTarget,
) -> String {
    let use_runtime_prop_normalization = exact_props_use_runtime_normalization(element, target);
    let mut merge_args = Vec::new();
    let mut object_entries = Vec::new();
    if let Some(key) = branch_key {
        object_entries.push(format!("key: {key}"));
    }

    for (prop_index, prop) in element.props.iter().enumerate() {
        if !exact_props_include_prop(prop, target) {
            continue;
        }
        match prop {
            Vue3Prop::Attribute(attr) => {
                object_entries.push(render_attribute_prop(element, attr));
            }
            Vue3Prop::Directive(dir) if dir.name == "on" && dir.arg.is_none() => {
                if let Some(listeners) = render_object_listeners_prop(element, dir, options, scope)
                {
                    push_exact_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(ExactPropsArg {
                        kind: ExactPropsArgKind::ObjectListeners,
                        code: listeners,
                    });
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "on" && dir.is_dynamic_arg => {
                push_exact_object_arg(&mut merge_args, &mut object_entries);
                merge_args.push(ExactPropsArg {
                    kind: ExactPropsArgKind::DynamicEvent,
                    code: render_plain_props(&[render_event_prop(
                        element, dir, options, scope, memo_index,
                    )])
                    .unwrap_or_else(|| "{}".into()),
                });
            }
            Vue3Prop::Directive(dir) if dir.name == "on" => {
                object_entries.push(render_event_prop(element, dir, options, scope, memo_index));
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" && dir.arg.is_none() => {
                if let Some(binding) = render_object_binding_prop(dir, options, scope) {
                    push_exact_object_arg(&mut merge_args, &mut object_entries);
                    merge_args.push(ExactPropsArg {
                        kind: ExactPropsArgKind::ObjectBinding,
                        code: binding,
                    });
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" => {
                if let Some(binding) = render_binding_prop(
                    node_id,
                    prop_index,
                    dir,
                    options,
                    scope,
                    use_runtime_prop_normalization,
                ) {
                    object_entries.push(binding);
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "html" || dir.name == "text" => {
                object_entries.push(render_content_directive_prop(dir, options, scope));
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && vue3_dom_model_kind(element).is_some() =>
            {
                object_entries.push(render_model_update_prop_exact(
                    dir, options, scope, memo_index,
                ));
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && element.tag_type == Vue3ElementType::Component =>
            {
                object_entries.extend(render_component_model_props(
                    dir, options, scope, memo_index,
                ));
            }
            _ => {}
        }
    }

    push_exact_object_arg(&mut merge_args, &mut object_entries);
    render_exact_props_args(
        merge_args,
        exact_props_has_dynamic_bind_arg(element, target),
    )
}

pub(crate) fn exact_props_include_prop(prop: &Vue3Prop, target: ExactPropsTarget) -> bool {
    if target != ExactPropsTarget::SlotOutlet {
        return true;
    }
    match prop {
        Vue3Prop::Attribute(attr) => attr.name != "name",
        Vue3Prop::Directive(dir) if dir.name == "bind" => {
            dir.is_dynamic_arg
                || dir
                    .arg
                    .as_ref()
                    .is_none_or(|arg| arg.source_string() != "name")
        }
        _ => true,
    }
}

pub(crate) fn exact_props_use_runtime_normalization(
    element: &Vue3Element,
    target: ExactPropsTarget,
) -> bool {
    element.props.iter().any(|prop| {
        exact_props_include_prop(prop, target)
            && matches!(
                prop,
                Vue3Prop::Directive(dir)
                    if matches!(dir.name.as_str(), "bind" | "on")
                        && (dir.is_dynamic_arg || dir.arg.is_none())
            )
    })
}

pub(crate) fn exact_props_has_dynamic_bind_arg(
    element: &Vue3Element,
    target: ExactPropsTarget,
) -> bool {
    element.props.iter().any(|prop| {
        exact_props_include_prop(prop, target)
            && matches!(
                prop,
                Vue3Prop::Directive(dir)
                    if dir.name == "bind" && dir.is_dynamic_arg && dir.arg.is_some()
            )
    })
}

pub(crate) fn push_exact_object_arg(
    merge_args: &mut Vec<ExactPropsArg>,
    object_entries: &mut Vec<String>,
) {
    if let Some(code) = render_plain_props(object_entries) {
        merge_args.push(ExactPropsArg {
            kind: ExactPropsArgKind::Object,
            code,
        });
        object_entries.clear();
    }
}

pub(crate) fn render_exact_props_args(
    args: Vec<ExactPropsArg>,
    has_dynamic_bind_arg: bool,
) -> String {
    match args.as_slice() {
        [] => String::new(),
        [arg] if arg.kind == ExactPropsArgKind::ObjectBinding => {
            format!("_normalizeProps(_guardReactiveProps({}))", arg.code)
        }
        [arg] if arg.kind == ExactPropsArgKind::Object && has_dynamic_bind_arg => {
            format!("_normalizeProps({})", arg.code)
        }
        [arg] if arg.kind == ExactPropsArgKind::DynamicEvent => arg.code.clone(),
        [arg] => arg.code.clone(),
        _ => format!(
            "_mergeProps({})",
            args.iter()
                .map(|arg| arg.code.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub(crate) fn render_plain_props(entries: &[String]) -> Option<String> {
    if entries.is_empty() {
        None
    } else if entries.len() == 1 && exact_single_prop_prefers_multiline(&entries[0]) {
        Some(render_object(entries))
    } else if entries.len() == 1 {
        Some(format!("{{ {} }}", entries.join(", ")))
    } else {
        Some(render_object(entries))
    }
}

pub(crate) fn render_static_props_hoist_object(
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let entries = element
        .props
        .iter()
        .enumerate()
        .filter_map(|(prop_index, prop)| match prop {
            Vue3Prop::Attribute(attr) => Some(render_attribute_prop(element, attr)),
            Vue3Prop::Directive(dir) if dir.name == "bind" => {
                render_static_binding_hoist_prop(node_id, prop_index, dir, options, scope)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    render_plain_props(&entries)
}

pub(crate) fn exact_single_prop_prefers_multiline(entry: &str) -> bool {
    entry.contains('\n')
        || entry.contains("_cache[")
        || entry.contains(": _toDisplayString(")
        || entry.contains(": _normalizeStyle(")
        || entry.starts_with("key: ") && entry.contains('(')
}

pub(crate) fn render_attribute_prop(
    element: &Vue3Element,
    attr: &vuec_ast::Vue3Attribute,
) -> String {
    match &attr.value {
        Some(value) if element.tag_type == Vue3ElementType::Element && attr.name == "style" => {
            format!("style: {}", vue3_static_style_object_expr(value))
        }
        Some(value) => format!("{}: {}", json_key(&attr.name), quote_string(value)),
        None if element.tag_type == Vue3ElementType::Element => {
            format!("{}: true", json_key(&attr.name))
        }
        None => format!("{}: {}", json_key(&attr.name), quote_string("")),
    }
}

pub(crate) fn render_object_binding_prop(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let value = dir.exp.as_ref()?.source_string();
    let value = value.trim();
    (!value.is_empty()).then(|| rewrite_expression_with_scope(value, options, scope))
}

pub(crate) fn render_object_listeners_prop(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let value = dir.exp.as_ref()?.source_string();
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let value = rewrite_expression_with_scope(value, options, scope);
    if element.tag_type == Vue3ElementType::Component {
        Some(format!("_toHandlers({value})"))
    } else {
        Some(format!("_toHandlers({value}, true)"))
    }
}

pub(crate) fn render_binding_prop(
    node_id: Option<vuec_ast::NodeId>,
    prop_index: usize,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    use_runtime_prop_normalization: bool,
) -> Option<String> {
    let arg = dir.arg.as_ref()?.source_string();
    let value = dir.exp.as_ref()?.source_string();
    let value = value.trim();
    if arg.is_empty() || value.is_empty() {
        return None;
    }
    if dir.is_dynamic_arg {
        let expression = rewrite_expression_with_scope(value, options, scope);
        let key = render_dynamic_binding_prop_key(dir, &arg, options, scope);
        return Some(format!("[{key}]: {expression}"));
    }
    let key = render_static_binding_prop_key(dir);
    let expression = node_id
        .and_then(|node_id| scope.static_hoists.binding_index(node_id, prop_index))
        .map(|index| format!("_hoisted_{index}"))
        .unwrap_or_else(|| rewrite_expression_with_scope(value, options, scope));
    if key == "class" && !use_runtime_prop_normalization {
        Some(format!("class: _normalizeClass({expression})"))
    } else if key == "style" && !use_runtime_prop_normalization {
        Some(format!("style: _normalizeStyle({expression})"))
    } else {
        Some(format!("{}: {}", json_key(&key), expression))
    }
}

pub(crate) fn render_static_binding_hoist_prop(
    node_id: vuec_ast::NodeId,
    prop_index: usize,
    dir: &Vue3Directive,
    _options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if dir.is_dynamic_arg || !dir.modifiers.is_empty() {
        return None;
    }
    let key = render_static_binding_prop_key(dir);
    let source = dir.exp.as_ref()?.source_string();
    let expression = if let Some(index) = scope.static_hoists.binding_index(node_id, prop_index) {
        format!("_hoisted_{index}")
    } else if expression_is_generated_asset_import(&source) {
        source.trim().to_string()
    } else {
        let value = static_const_eval_source(&source)?;
        match value {
            StaticConstValue::String(value) => quote_string(&value),
            StaticConstValue::Number(value) => value,
            StaticConstValue::Bool(value) => value.to_string(),
            StaticConstValue::Null => "null".into(),
            StaticConstValue::Array(_) | StaticConstValue::Object(_) => return None,
        }
    };
    if key == "class" {
        Some(format!("class: _normalizeClass({expression})"))
    } else if key == "style" {
        Some(format!("style: _normalizeStyle({expression})"))
    } else {
        Some(format!("{}: {}", json_key(&key), expression))
    }
}

pub(crate) fn render_content_directive_prop(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let value = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let value = value.trim();
    let key = if dir.name == "html" {
        "innerHTML"
    } else {
        "textContent"
    };
    let expression = if value.is_empty() {
        quote_string("")
    } else {
        let expression = rewrite_expression_with_scope(value, options, scope);
        if dir.name == "text" && !content_directive_text_is_static(dir, options) {
            format!("_toDisplayString({expression})")
        } else {
            expression
        }
    };
    format!("{}: {}", json_key(key), expression)
}

pub(crate) fn render_model_update_prop_exact(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    format!(
        "{}: {}",
        json_key("onUpdate:modelValue"),
        render_model_assignment_for_directive_cached(dir, options, scope, memo_index)
    )
}

pub(crate) fn render_component_model_props(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let prop_name = component_model_prop_name(dir);
    let value = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let value = rewrite_expression_with_scope(&value, options, scope);
    let mut props = vec![format!("{}: {}", json_key(&prop_name), value)];
    props.push(render_component_model_update_prop(
        dir, options, scope, memo_index,
    ));
    let modifiers = component_model_modifiers_prop(dir);
    if !modifiers.is_empty() {
        props.push(modifiers);
    }
    props
}

pub(crate) fn component_model_prop_name(dir: &Vue3Directive) -> String {
    dir.arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|arg| !arg.trim().is_empty())
        .map(|arg| arg.trim().to_string())
        .unwrap_or_else(|| "modelValue".into())
}

pub(crate) fn render_component_model_update_prop(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    format!(
        "{}: {}",
        json_key(&format!(
            "onUpdate:{}",
            camelize(&component_model_prop_name(dir))
        )),
        render_model_assignment_for_directive_cached(dir, options, scope, memo_index)
    )
}

pub(crate) fn component_model_modifiers_prop(dir: &Vue3Directive) -> String {
    if dir.modifiers.is_empty() {
        return String::new();
    }
    let prop_name = if dir.arg.is_some() {
        format!("{}Modifiers", component_model_prop_name(dir))
    } else {
        "modelModifiers".into()
    };
    let entries = dir
        .modifiers
        .iter()
        .map(|modifier| format!("{modifier}: true"))
        .collect::<Vec<_>>();
    format!("{}: {}", json_key(&prop_name), render_object(&entries))
}

pub(crate) fn render_model_assignment_for_directive(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
) -> String {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    render_inline_model_assignment(
        raw,
        "$event",
        options.binding_metadata.get(raw).map(String::as_str),
        options,
        || rewrite_expression_with_scope(raw, options, &RenderScope::default()),
    )
}

pub(crate) fn render_model_assignment_for_directive_cached(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    let mut assignment = render_inline_model_assignment(
        raw,
        "$event",
        options.binding_metadata.get(raw).map(String::as_str),
        options,
        || rewrite_expression_with_scope(raw, options, scope),
    );
    if should_cache_model_update_exact(raw, options, scope) {
        let index = memo_index.alloc();
        assignment = format!("_cache[{index}] || (_cache[{index}] = {assignment})");
    }
    assignment
}

pub(crate) fn should_cache_model_update_exact(
    raw: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> bool {
    options.cache_handlers
        && uses_prefixed_identifiers(options)
        && !scope.in_v_once
        && !event_handler_has_scope_ref(raw, scope)
}

pub(crate) fn render_static_binding_prop_key(dir: &Vue3Directive) -> String {
    let mut key = dir
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    if dir.modifiers.iter().any(|modifier| modifier == "camel") {
        key = camelize(&key);
    }
    if dir.modifiers.iter().any(|modifier| modifier == "prop") {
        key = format!(".{key}");
    } else if dir.modifiers.iter().any(|modifier| modifier == "attr") {
        key = format!("^{key}");
    }
    key
}

pub(crate) fn render_dynamic_binding_prop_key(
    dir: &Vue3Directive,
    arg: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let mut key = render_dynamic_prop_key(&rewrite_expression_with_scope(arg, options, scope));
    if dir.modifiers.iter().any(|modifier| modifier == "camel") {
        key = format!("_camelize({key})");
    }
    if dir.modifiers.iter().any(|modifier| modifier == "prop") {
        key = format!("'.' + ({key})");
    } else if dir.modifiers.iter().any(|modifier| modifier == "attr") {
        key = format!("'^' + ({key})");
    }
    key
}

pub(crate) fn render_event_prop(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let key = render_event_prop_key(element, dir, options, scope);
    let value = render_event_handler_value(element, dir, options, scope, memo_index);
    format!("{key}: {value}")
}

pub(crate) fn render_event_prop_key(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if dir.is_dynamic_arg {
        let event = dir
            .arg
            .as_ref()
            .map(Vue3Expression::source_string)
            .unwrap_or_default();
        let event = rewrite_expression_with_scope(&event, options, scope);
        let event = format!("_toHandlerKey({})", event.trim());
        return format!("[{event}]");
    }

    let event = dir
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    json_key(&event_handler_prop_name(element, &event))
}

pub(crate) fn render_event_handler_value(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    let is_member = transform_on_is_member_expression(
        raw,
        &json!({
            "allowLexerFallback": true,
            "isTS": options.is_ts,
            "expressionPlugins": options.expression_plugins,
        }),
    );
    let is_fn = transform_on_is_fn_expression(
        raw,
        &json!({
            "isTS": options.is_ts,
            "expressionPlugins": options.expression_plugins,
        }),
    );
    let is_inline = !is_member && !is_fn;
    let mut handler = if raw.is_empty() {
        "() => {}".into()
    } else if is_inline {
        let value = rewrite_handler_expression_with_scope(raw, options, scope);
        let has_multiple_statements = raw.contains(';');
        if has_multiple_statements {
            format!("$event => {{{value}}}")
        } else {
            format!("$event => ({value})")
        }
    } else {
        rewrite_handler_expression_with_scope(raw, options, scope)
    };

    let should_cache =
        should_cache_event_handler(element, dir, options, scope, raw, is_member, is_inline);
    if should_cache && is_member {
        let value = rewrite_handler_expression_with_scope(raw, options, scope);
        handler = format!("(...args) => ({value} && {value}(...args))");
    }
    if should_cache {
        let index = memo_index.alloc();
        if handler.contains('\n') {
            handler = dedent_after_first_line(&handler, 2);
        }
        handler = format!("_cache[{index}] || (_cache[{index}] = {handler})");
    }
    handler
}

pub(crate) fn should_cache_event_handler(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    raw: &str,
    is_member: bool,
    is_inline: bool,
) -> bool {
    if !options.cache_handlers || scope.in_v_once {
        return false;
    }
    if !uses_prefixed_identifiers(options) {
        return false;
    }
    if raw.is_empty() {
        return true;
    }
    if element.tag_type == Vue3ElementType::Component && is_member {
        return false;
    }
    if event_handler_has_scope_ref(raw, scope) {
        return false;
    }
    if !is_inline && event_handler_is_const_binding(raw, options) {
        return false;
    }
    if is_inline && vue3_for_const_type(raw) > 0 {
        return false;
    }
    if dir.is_dynamic_arg {
        return true;
    }
    true
}

pub(crate) fn event_handler_has_scope_ref(raw: &str, scope: &RenderScope) -> bool {
    scope
        .locals
        .iter()
        .any(|local| source_contains_identifier(raw, local))
}

pub(crate) fn event_handler_is_const_binding(raw: &str, options: &Vue3CompilerOptions) -> bool {
    let trimmed = raw.trim();
    is_simple_identifier_ascii(trimmed)
        && matches!(
            options.binding_metadata.get(trimmed).map(String::as_str),
            Some("setup-const" | "literal-const")
        )
}

pub(crate) fn render_object(properties: &[String]) -> String {
    if properties.is_empty() {
        "{}".into()
    } else {
        format!(
            "{{\n{}\n}}",
            properties
                .iter()
                .map(|property| indent_lines(property, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

pub(crate) fn render_inline_array(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

pub(crate) fn has_class_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
        )
    })
}

pub(crate) fn has_dynamic_non_key_props(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir) if prop_requires_dynamic_patch(element, dir, options, scope)
        )
    })
}

pub(crate) fn prop_requires_dynamic_patch(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> bool {
    if static_cached_binding_is_constant(dir) {
        return false;
    }
    if dir.name == "model" && element.tag_type == Vue3ElementType::Component {
        return true;
    }
    if dir.name == "model" && vue3_dom_model_kind(element).is_some() {
        return !native_model_update_can_skip_patch(dir, options, scope);
    }
    if dir.name == "html" || dir.name == "text" {
        return true;
    }
    if dir.name == "on" && !event_directive_is_vnode_hook(dir) {
        return !event_handler_can_skip_patch(element, dir, options, scope);
    }
    if dir.name != "bind" || is_asset_import_binding(dir) {
        return false;
    }
    let Some(arg) = dir.arg.as_ref().map(Vue3Expression::source_string) else {
        return true;
    };
    if arg == "key" {
        return false;
    }
    if element.tag_type == Vue3ElementType::Element && matches!(arg.as_str(), "class" | "style") {
        return false;
    }
    true
}

pub(crate) fn static_cached_binding_is_constant(dir: &Vue3Directive) -> bool {
    dir.name == "bind"
        && !dir.is_dynamic_arg
        && dir
            .arg
            .as_ref()
            .is_some_and(|arg| arg.source_string() != "key")
        && dir
            .exp
            .as_ref()
            .is_some_and(|exp| static_const_eval_source(&exp.source_string()).is_some())
}

pub(crate) fn static_cached_bindings_are_constant(element: &Vue3Element) -> bool {
    element.props.iter().all(|prop| match prop {
        Vue3Prop::Attribute(_) => true,
        Vue3Prop::Directive(dir) if dir.name == "bind" => {
            !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() != "key")
                && dir
                    .exp
                    .as_ref()
                    .is_some_and(|exp| static_const_eval_source(&exp.source_string()).is_some())
        }
        Vue3Prop::Directive(dir) => matches!(dir.name.as_str(), "once" | "memo"),
    })
}

pub(crate) fn event_handler_can_skip_patch(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> bool {
    if !dir.is_dynamic_arg
        && dir.arg.is_some()
        && event_handler_is_const_binding(
            &dir.exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default(),
            options,
        )
    {
        return true;
    }
    if !options.cache_handlers || !uses_prefixed_identifiers(options) {
        return false;
    }
    if directive_by_name(element, "once").is_some() {
        return false;
    }
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    let is_member = transform_on_is_member_expression(
        raw,
        &json!({
            "allowLexerFallback": true,
            "isTS": options.is_ts,
            "expressionPlugins": options.expression_plugins,
        }),
    );
    let is_fn = transform_on_is_fn_expression(
        raw,
        &json!({
            "isTS": options.is_ts,
            "expressionPlugins": options.expression_plugins,
        }),
    );
    let is_inline = !is_member && !is_fn;
    if !should_cache_event_handler(element, dir, options, scope, raw, is_member, is_inline) {
        return false;
    }
    if element.tag_type == Vue3ElementType::Component && is_member {
        return false;
    }
    true
}

pub(crate) fn has_vnode_hook(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir) if dir.name == "on" && event_directive_is_vnode_hook(dir)
        )
    })
}

pub(crate) fn has_runtime_directive(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir) if vue3_directive_needs_runtime_asset(&dir.name)
        )
    })
}

pub(crate) fn dynamic_props_arg(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" && !event_directive_is_vnode_hook(dir) => {
                if dir.is_dynamic_arg || dir.arg.is_none() {
                    return None;
                }
                if event_handler_can_skip_patch(element, dir, options, scope) {
                    return None;
                }
                let event = dir
                    .arg
                    .as_ref()
                    .map(Vue3Expression::source_string)
                    .unwrap_or_default();
                Some(event_handler_prop_name(element, &event))
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" && !has_key_bind_dir(dir) => {
                if is_asset_import_binding(dir) || dir.is_dynamic_arg {
                    return None;
                }
                if static_cached_binding_is_constant(dir) {
                    return None;
                }
                let arg = dir.arg.as_ref()?.source_string();
                if arg.is_empty()
                    || element.tag_type == Vue3ElementType::Element
                        && matches!(arg.as_str(), "class" | "style")
                {
                    return None;
                }
                (!arg.is_empty()).then_some(render_static_binding_prop_key(dir))
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && element.tag_type == Vue3ElementType::Component =>
            {
                Some(component_model_prop_name(dir))
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && vue3_dom_model_kind(element).is_some() =>
            {
                (!native_model_update_can_skip_patch(dir, options, scope))
                    .then_some("onUpdate:modelValue".into())
            }
            Vue3Prop::Directive(dir) if dir.name == "html" => Some("innerHTML".into()),
            Vue3Prop::Directive(dir) if dir.name == "text" => Some("textContent".into()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if props.is_empty() {
        String::new()
    } else {
        format!(
            ", [{}]",
            props
                .iter()
                .map(|prop| quote_string(prop))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

pub(crate) fn native_model_update_can_skip_patch(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> bool {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    should_cache_model_update_exact(raw.trim(), options, scope)
}

pub(crate) fn event_directive_is_vnode_hook(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string().starts_with("vue:"))
}

pub(crate) fn exact_content_directive(element: &Vue3Element) -> Option<&Vue3Directive> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir) if dir.name == "html" || dir.name == "text" => Some(dir),
        _ => None,
    })
}

pub(crate) fn render_static_content_directive_child(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
) -> Option<String> {
    let dir = exact_content_directive(element)?;
    if !content_directive_text_is_static(dir, options) {
        return None;
    }
    let source = dir.exp.as_ref()?.source_string();
    let value = static_const_eval_source(&source)?.to_display_string()?;
    Some(if dir.name == "text" {
        quote_string(&value)
    } else {
        quote_string(&decode_static_html_entities(&value))
    })
}

pub(crate) fn content_directive_text_is_static(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
) -> bool {
    let Some(source) = dir.exp.as_ref().map(Vue3Expression::source_string) else {
        return true;
    };
    let source = source.trim();
    process_expression_is_static_literal(source)
        || matches!(
            options.binding_metadata.get(source).map(String::as_str),
            Some("literal-const")
        )
        || vue3_expression_is_string_literal(source)
}

pub(crate) fn event_handler_prop_name(element: &Vue3Element, event: &str) -> String {
    let raw_name = if let Some(hook) = event.strip_prefix("vue:") {
        format!("vnode-{hook}")
    } else {
        event.to_string()
    };
    if element.tag_type != Vue3ElementType::Element
        || raw_name.starts_with("vnode")
        || !raw_name.chars().any(|ch| ch.is_ascii_uppercase())
    {
        format!("on{}", capitalize(&camelize(&raw_name)))
    } else {
        format!("on:{raw_name}")
    }
}

pub(crate) fn event_handler_prop_name_for_component(event: &str) -> String {
    let raw_name = event
        .strip_prefix("vue:")
        .map(|hook| format!("vnode-{hook}"))
        .unwrap_or_else(|| event.to_string());
    format!("on{}", capitalize(&camelize(&raw_name)))
}

pub(crate) fn event_handler_prop_name_for_element(event: &str) -> String {
    let raw_name = event
        .strip_prefix("vue:")
        .map(|hook| format!("vnode-{hook}"))
        .unwrap_or_else(|| event.to_string());
    if raw_name.starts_with("vnode") || !raw_name.chars().any(|ch| ch.is_ascii_uppercase()) {
        format!("on{}", capitalize(&camelize(&raw_name)))
    } else {
        format!("on:{raw_name}")
    }
}

pub(crate) fn has_class_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "class")
}

pub(crate) fn has_key_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "key")
}

pub(crate) fn vue3_prop_is_vnode_cacheable_static(prop: &Vue3Prop) -> bool {
    match prop {
        Vue3Prop::Attribute(_) => true,
        Vue3Prop::Directive(dir) => {
            is_asset_import_binding(dir)
                || dir.name == "bind"
                    && !dir.is_dynamic_arg
                    && dir.modifiers.is_empty()
                    && dir.arg.is_some()
                    && dir
                        .exp
                        .as_ref()
                        .is_some_and(|exp| static_const_eval_source(&exp.source_string()).is_some())
        }
    }
}

pub(crate) fn vue3_prop_is_static_cacheable_for_hoist(prop: &Vue3Prop) -> bool {
    match prop {
        Vue3Prop::Attribute(_) => true,
        Vue3Prop::Directive(dir) => {
            is_asset_import_binding(dir)
                || dir.name == "bind"
                    && !dir.is_dynamic_arg
                    && dir.modifiers.is_empty()
                    && dir.arg.is_some()
                    && dir
                        .exp
                        .as_ref()
                        .is_some_and(|exp| static_const_eval_source(&exp.source_string()).is_some())
        }
    }
}

pub(crate) fn vue3_prop_is_static_cacheable_for_ns(
    prop: &Vue3Prop,
    ns: vuec_ast::HtmlNamespace,
) -> bool {
    match prop {
        Vue3Prop::Attribute(attr) => static_html_attr_is_stringifiable(&attr.name, ns),
        Vue3Prop::Directive(dir) => {
            is_asset_import_binding(dir) || static_html_directive_is_stringifiable_static(dir, ns)
        }
    }
}

pub(crate) fn static_html_directive_is_stringifiable_static(
    dir: &Vue3Directive,
    ns: vuec_ast::HtmlNamespace,
) -> bool {
    match dir.name.as_str() {
        "bind" => {
            !dir.is_dynamic_arg
                && dir.modifiers.is_empty()
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| static_html_attr_is_stringifiable(&arg.source_string(), ns))
                && dir
                    .exp
                    .as_ref()
                    .is_some_and(|exp| static_const_eval_source(&exp.source_string()).is_some())
        }
        "html" | "text" => dir.exp.as_ref().is_some_and(|exp| {
            static_const_eval_source(&exp.source_string())
                .and_then(|value| value.to_display_string())
                .is_some()
        }),
        _ => false,
    }
}

pub(crate) fn is_asset_import_binding(dir: &Vue3Directive) -> bool {
    dir.name == "bind"
        && !dir.is_dynamic_arg
        && dir.arg.is_some()
        && dir
            .exp
            .as_ref()
            .is_some_and(|exp| expression_is_generated_asset_import(&exp.source_string()))
}

pub(crate) fn expression_is_generated_asset_import(expression: &str) -> bool {
    generated_asset_import_expression_parts(expression).is_some()
}

pub(crate) fn generated_asset_import_expression_has_literal(expression: &str) -> bool {
    generated_asset_import_expression_parts(expression).is_some_and(|parts| {
        parts
            .iter()
            .any(|part| matches!(part, AssetImportExpressionPart::Literal(_)))
    })
}

pub(crate) fn static_html_asset_import_expression(expression: &str) -> Option<StaticHtmlBuffer> {
    let mut html = StaticHtmlBuffer::default();
    for part in generated_asset_import_expression_parts(expression)? {
        match part {
            AssetImportExpressionPart::Import(value) => html.push_expression(value),
            AssetImportExpressionPart::Literal(value) => {
                html.push_text(escape_static_html_attr(&value));
            }
        }
    }
    Some(html)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AssetImportExpressionPart {
    Import(String),
    Literal(String),
}

pub(crate) fn generated_asset_import_expression_parts(
    expression: &str,
) -> Option<Vec<AssetImportExpressionPart>> {
    let parts = split_top_level_like(expression, '+');
    if parts.is_empty() {
        return None;
    }
    let parts = parts
        .into_iter()
        .map(|part| {
            let part = part.trim();
            if is_generated_asset_import_ident(part) {
                Some(AssetImportExpressionPart::Import(part.to_string()))
            } else if quoted_js_literal(part) {
                match static_const_eval_source(part)? {
                    StaticConstValue::String(value) => {
                        Some(AssetImportExpressionPart::Literal(value))
                    }
                    _ => None,
                }
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>()?;
    parts
        .iter()
        .any(|part| matches!(part, AssetImportExpressionPart::Import(_)))
        .then_some(parts)
}

pub(crate) fn quoted_js_literal(value: &str) -> bool {
    vue3_expression_is_string_literal(value)
}

pub(crate) fn directive_by_name<'a>(
    element: &'a Vue3Element,
    name: &str,
) -> Option<&'a Vue3Directive> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir) if dir.name == name => Some(dir),
        _ => None,
    })
}

pub(crate) fn is_else_branch(element: &Vue3Element) -> bool {
    directive_by_name(element, "else").is_some() || directive_by_name(element, "else-if").is_some()
}

pub(crate) fn parse_v_for_expression(expression: &str) -> Option<(String, Vec<String>)> {
    let expression = expression.trim();
    let (raw_aliases, source) = expression
        .split_once(" in ")
        .or_else(|| expression.split_once(" of "))?;
    let raw_aliases = raw_aliases
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    let aliases = split_top_level_like(raw_aliases, ',')
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        None
    } else {
        Some((source.trim().to_string(), aliases))
    }
}

pub(crate) fn normalize_v_for_aliases(aliases: &[String]) -> Vec<String> {
    aliases
        .iter()
        .flat_map(|alias| extract_v_for_alias_locals(alias))
        .collect()
}

pub(crate) fn extract_v_for_alias_locals(alias: &str) -> Vec<String> {
    let alias = alias.trim();
    if alias.starts_with('{') || alias.starts_with('[') {
        return extract_destructure_alias_locals(alias);
    }
    if alias
        .chars()
        .next()
        .is_some_and(|ch| is_identifier_start(ch))
    {
        vec![alias.to_string()]
    } else {
        Vec::new()
    }
}

pub(crate) fn extract_destructure_alias_locals(alias: &str) -> Vec<String> {
    let trimmed = alias
        .trim()
        .trim_start_matches('{')
        .trim_start_matches('[')
        .trim_end_matches('}')
        .trim_end_matches(']');
    split_top_level_like(trimmed, ',')
        .into_iter()
        .flat_map(|part| extract_slot_params(part))
        .collect()
}

pub(crate) fn split_top_level_like(source: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut quote = None;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 => {
                let item = source[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let item = source[start..].trim();
    if !item.is_empty() {
        items.push(item);
    }
    items
}

pub(crate) fn find_top_level_char(source: &str, target: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escape = false;
    for (index, ch) in source.char_indices() {
        if let Some(active_quote) = quote {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == target && depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

pub(crate) fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.collect::<String>()
    )
}

pub(crate) fn rewrite_handler_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let scope = scope.with_locals(vec!["$event".into()]);
    normalize_handler_indent(&rewrite_expression_with_scope(expression, options, &scope))
}

pub(crate) fn rewrite_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = expression.trim();
    if !uses_prefixed_identifiers(options) {
        return expression.to_string();
    }
    if scope.locals.is_empty() {
        rewrite_js_like_expression(expression, options)
    } else {
        rewrite_js_like_expression_with_locals(expression, options, &scope.locals)
    }
}

pub(crate) fn rewrite_ssr_css_vars_expression(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let trimmed = expression.trim();
    let Some(body) = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return rewrite_expression_with_scope(trimmed, options, scope);
    };
    let multiline = body.contains('\n');
    let properties = split_top_level_like(body, ',')
        .into_iter()
        .map(|property| rewrite_ssr_css_var_property(property, options, scope))
        .collect::<Vec<_>>();
    if multiline {
        return format!("{{\n  {}\n}}", properties.join(",\n  "));
    }
    format!("{{ {} }}", properties.join(", "))
}

pub(crate) fn rewrite_ssr_css_var_property(
    property: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let property = property.trim();
    if property.starts_with("...") {
        return rewrite_expression_with_scope(property, options, scope);
    }
    if let Some(colon) = find_top_level_char(property, ':') {
        let key = property[..colon].trim();
        let value = property[colon + 1..].trim();
        return format!(
            "{key}: {}",
            rewrite_expression_with_scope(value, options, scope)
        );
    }
    if is_simple_identifier(property) {
        format!(
            "{property}: {}",
            rewrite_identifier_with_scope(property, options, scope)
        )
    } else {
        rewrite_expression_with_scope(property, options, scope)
    }
}

pub(crate) fn rewrite_expression_with_scope_preserve_outer(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if !uses_prefixed_identifiers(options) {
        return expression.to_string();
    }
    if scope.locals.is_empty() {
        rewrite_js_like_expression(expression, options)
    } else {
        rewrite_js_like_expression_with_locals(expression, options, &scope.locals)
    }
}

pub(crate) fn rewrite_identifier_with_scope(
    ident: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if !uses_prefixed_identifiers(options) || scope.locals.iter().any(|local| local == ident) {
        ident.to_string()
    } else {
        rewrite_identifier(ident, options)
    }
}

pub(crate) fn uses_prefixed_identifiers(options: &Vue3CompilerOptions) -> bool {
    options.prefix_identifiers || options.mode == "module"
}

pub(crate) fn normalize_handler_indent(expression: &str) -> String {
    let mut lines = expression.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut normalized = String::from(first);
    for line in lines {
        normalized.push('\n');
        normalized.push_str(
            line.strip_prefix("    ")
                .or_else(|| line.strip_prefix("  "))
                .unwrap_or(line),
        );
    }
    normalized
}

pub(crate) fn rewrite_js_like_expression(
    expression: &str,
    options: &Vue3CompilerOptions,
) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, Vec::new(), &mut output);
    output
}

pub(crate) fn rewrite_js_like_expression_with_locals(
    expression: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, locals.to_vec(), &mut output);
    output
}

pub(crate) fn rewrite_js_like_expression_into(
    expression: &str,
    options: &Vue3CompilerOptions,
    root_locals: Vec<String>,
    output: &mut String,
) {
    let mut scopes = vec![Scope {
        locals: root_locals,
    }];
    let mut previous = TokenKind::Other;
    let mut pending_decl: Option<DeclKind> = None;
    let mut pending_function_params = false;
    let mut last_keyword: Option<String> = None;
    let mut paren_depth = 0usize;
    let mut for_pending = false;
    let mut for_header_depth: Option<usize> = None;
    let mut pending_for_block_locals = Vec::<String>::new();
    let mut catch_pending = false;
    let mut catch_param_depth: Option<usize> = None;
    let mut pending_catch_locals = Vec::<String>::new();
    let chars = expression.char_indices().collect::<Vec<_>>();
    let arrow_bindings = process_expression_arrow_bindings(expression);
    let mut index = 0usize;
    while index < chars.len() {
        let byte = chars[index].0;
        let ch = chars[index].1;
        if ch == '/' {
            if let Some(tail) = expression.get(byte..) {
                if tail.starts_with("//") {
                    while index < chars.len() {
                        let current = chars[index].1;
                        output.push(current);
                        index += 1;
                        if current == '\n' || current == '\r' {
                            break;
                        }
                    }
                    previous = TokenKind::Other;
                    continue;
                }
                if tail.starts_with("/*") {
                    output.push('/');
                    output.push('*');
                    index += 2;
                    while index < chars.len() {
                        let current = chars[index].1;
                        output.push(current);
                        index += 1;
                        if current == '*' && index < chars.len() && chars[index].1 == '/' {
                            output.push('/');
                            index += 1;
                            break;
                        }
                    }
                    previous = TokenKind::Other;
                    continue;
                }
            }
        }
        if ch == '`' {
            index =
                rewrite_template_literal_into(expression, &chars, index, options, &scopes, output);
            previous = TokenKind::Other;
            continue;
        }
        if ch == '\'' || ch == '"' {
            let quote = ch;
            output.push(ch);
            index += 1;
            while index < chars.len() {
                let current = chars[index].1;
                output.push(current);
                index += 1;
                if current == '\\' && index < chars.len() {
                    output.push(chars[index].1);
                    index += 1;
                    continue;
                }
                if current == quote {
                    break;
                }
            }
            previous = TokenKind::Other;
            continue;
        }
        if (ch == '+' || ch == '-')
            && expression
                .get(byte..)
                .is_some_and(|tail| tail.starts_with("++") || tail.starts_with("--"))
        {
            let operator = if ch == '+' { "++" } else { "--" };
            let ident_start = skip_ws_forward(expression, byte + operator.len());
            if let Some((ident, ident_end)) = read_identifier_at(expression, ident_start) {
                if let Some((replacement, consumed_end)) = rewrite_js_like_update(
                    ident,
                    expression,
                    ident_start,
                    ident_end,
                    options,
                    &scopes,
                    process_expression_is_arrow_param(
                        &arrow_bindings,
                        ident,
                        ident_start,
                        ident_end,
                    ),
                ) {
                    output.push_str(&replacement);
                    index = chars
                        .iter()
                        .position(|(offset, _)| *offset >= consumed_end)
                        .unwrap_or(chars.len());
                    previous = TokenKind::Other;
                    continue;
                }
            }
        }
        if is_identifier_start(ch) {
            let start = byte;
            index += 1;
            while index < chars.len() && is_identifier_continue(chars[index].1) {
                index += 1;
            }
            let end = chars
                .get(index)
                .map_or(expression.len(), |(offset, _)| *offset);
            let ident = &expression[start..end];
            let arrow_param = process_expression_is_arrow_param(&arrow_bindings, ident, start, end);
            let arrow_local = process_expression_is_arrow_local(&arrow_bindings, ident, start, end);
            let next = next_non_ws(expression, end);
            let prev = previous;
            if !process_expression_update_argument(expression, start, end)
                .is_some_and(|update| update.prefix)
            {
                if let Some((replacement, consumed_end)) = rewrite_js_like_update(
                    ident,
                    expression,
                    start,
                    end,
                    options,
                    &scopes,
                    arrow_param,
                ) {
                    output.push_str(&replacement);
                    index = chars
                        .iter()
                        .position(|(offset, _)| *offset >= consumed_end)
                        .unwrap_or(chars.len());
                    previous = TokenKind::Other;
                    continue;
                }
            }
            if let Some((replacement, consumed_end)) = rewrite_js_like_destructure_identifier(
                ident,
                expression,
                start,
                end,
                options,
                &scopes,
                arrow_param,
            ) {
                output.push_str(&replacement);
                index = chars
                    .iter()
                    .position(|(offset, _)| *offset >= consumed_end)
                    .unwrap_or(chars.len());
                previous = TokenKind::Other;
                continue;
            }
            if let Some((replacement, consumed_end)) = rewrite_js_like_assignment(
                ident,
                expression,
                start,
                end,
                options,
                &scopes,
                arrow_param,
            ) {
                output.push_str(&replacement);
                index = chars
                    .iter()
                    .position(|(offset, _)| *offset >= consumed_end)
                    .unwrap_or(chars.len());
                previous = TokenKind::Other;
                continue;
            }
            if is_keyword(ident) {
                output.push_str(ident);
                match ident {
                    "var" => pending_decl = Some(DeclKind::Var),
                    "let" | "const" => pending_decl = Some(DeclKind::Block),
                    "function" => pending_function_params = true,
                    "for" => for_pending = true,
                    "in" | "of" => pending_decl = None,
                    "catch" => catch_pending = true,
                    _ => {}
                }
                last_keyword = Some(ident.to_string());
                previous = TokenKind::Keyword;
                continue;
            }
            if catch_param_depth.is_some() {
                if next != Some(':') {
                    pending_catch_locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            if pending_decl.is_some()
                && matches!(
                    prev,
                    TokenKind::Keyword | TokenKind::Comma | TokenKind::OpenParen
                )
            {
                if pending_decl == Some(DeclKind::Var) {
                    if let Some(scope) = scopes.first_mut() {
                        scope.locals.push(ident.to_string());
                    }
                } else if for_header_depth.is_some() {
                    pending_for_block_locals.push(ident.to_string());
                } else if let Some(scope) = scopes.last_mut() {
                    scope.locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            let skip_property = matches!(prev, TokenKind::Dot)
                || (next == Some(':') && last_keyword.as_deref() != Some("case"))
                || (pending_function_params
                    && matches!(prev, TokenKind::OpenParen | TokenKind::Comma));
            if skip_property
                || is_global_or_literal(ident)
                || is_generated_asset_import_ident(ident)
                || is_local(&scopes, ident)
                || arrow_param
                || arrow_local
                || pending_for_block_locals.iter().any(|local| local == ident)
            {
                output.push_str(ident);
            } else {
                let content = rewrite_identifier(ident, options);
                output.push_str(&parenthesize_rewritten_identifier_for_new_expression(
                    expression, start, end, &content,
                ));
            }
            previous = TokenKind::Identifier;
            continue;
        }
        output.push(ch);
        match ch {
            '{' => {
                if !pending_for_block_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_for_block_locals),
                    });
                } else if !pending_catch_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_catch_locals),
                    });
                } else {
                    scopes.push(Scope::default());
                }
                previous = TokenKind::Other;
            }
            '}' => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
                previous = TokenKind::Other;
            }
            '(' => {
                paren_depth += 1;
                if for_pending {
                    for_header_depth = Some(paren_depth);
                    for_pending = false;
                }
                if catch_pending {
                    catch_param_depth = Some(paren_depth);
                    catch_pending = false;
                }
                previous = TokenKind::OpenParen;
            }
            ')' => {
                if catch_param_depth == Some(paren_depth) {
                    catch_param_depth = None;
                }
                if for_header_depth == Some(paren_depth) {
                    for_header_depth = None;
                }
                paren_depth = paren_depth.saturating_sub(1);
                pending_function_params = false;
                previous = TokenKind::Other;
            }
            ',' => previous = TokenKind::Comma,
            '.' => previous = TokenKind::Dot,
            ';' => {
                pending_decl = None;
                previous = TokenKind::Other;
            }
            _ if ch.is_whitespace() => {}
            _ => {
                if ch != ':' {
                    last_keyword = None;
                }
                previous = TokenKind::Other;
            }
        }
        index += 1;
    }
}

pub(crate) fn rewrite_template_literal_into(
    expression: &str,
    chars: &[(usize, char)],
    mut index: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    output: &mut String,
) -> usize {
    output.push('`');
    index += 1;
    while index < chars.len() {
        let ch = chars[index].1;
        output.push(ch);
        index += 1;
        if ch == '\\' && index < chars.len() {
            output.push(chars[index].1);
            index += 1;
            continue;
        }
        if ch == '`' {
            break;
        }
        if ch == '$' && index < chars.len() && chars[index].1 == '{' {
            if let Some(close) = find_template_literal_expression_close(expression, chars, index) {
                output.push('{');
                let inner_start = chars[index].0 + '{'.len_utf8();
                let inner_end = chars[close].0;
                if let Some(inner) = expression.get(inner_start..inner_end) {
                    let locals = scopes
                        .iter()
                        .flat_map(|scope| scope.locals.iter().cloned())
                        .collect::<Vec<_>>();
                    rewrite_js_like_expression_into(inner, options, locals, output);
                }
                output.push('}');
                index = close + 1;
            }
        }
    }
    index
}

pub(crate) fn find_template_literal_expression_close(
    expression: &str,
    chars: &[(usize, char)],
    mut index: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    while index < chars.len() {
        let byte = chars[index].0;
        let ch = chars[index].1;
        if ch == '\'' || ch == '"' {
            index = skip_quoted_chars(chars, index, ch);
            continue;
        }
        if ch == '`' {
            index = skip_template_literal_chars(expression, chars, index);
            continue;
        }
        if ch == '/'
            && expression
                .get(byte..)
                .is_some_and(|tail| tail.starts_with("//"))
        {
            index += 2;
            while index < chars.len() && !matches!(chars[index].1, '\n' | '\r') {
                index += 1;
            }
            continue;
        }
        if ch == '/'
            && expression
                .get(byte..)
                .is_some_and(|tail| tail.starts_with("/*"))
        {
            index += 2;
            while index < chars.len() {
                if chars[index].1 == '*' && index + 1 < chars.len() && chars[index + 1].1 == '/' {
                    index += 2;
                    break;
                }
                index += 1;
            }
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

pub(crate) fn skip_quoted_chars(chars: &[(usize, char)], mut index: usize, quote: char) -> usize {
    index += 1;
    while index < chars.len() {
        let ch = chars[index].1;
        index += 1;
        if ch == '\\' && index < chars.len() {
            index += 1;
            continue;
        }
        if ch == quote {
            break;
        }
    }
    index
}

pub(crate) fn skip_template_literal_chars(
    expression: &str,
    chars: &[(usize, char)],
    mut index: usize,
) -> usize {
    index += 1;
    while index < chars.len() {
        let ch = chars[index].1;
        index += 1;
        if ch == '\\' && index < chars.len() {
            index += 1;
            continue;
        }
        if ch == '`' {
            break;
        }
        if ch == '$' && index < chars.len() && chars[index].1 == '{' {
            if let Some(close) = find_template_literal_expression_close(expression, chars, index) {
                index = close + 1;
            }
        }
    }
    index
}

pub(crate) fn rewrite_js_like_assignment(
    ident: &str,
    expression: &str,
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    arrow_local: bool,
) -> Option<(String, usize)> {
    if !options.inline || is_local(scopes, ident) || arrow_local {
        return None;
    }
    let binding = options.binding_metadata.get(ident).map(String::as_str)?;
    let assignment = process_expression_assignment_rhs(expression, start, end)?;
    let operator_start = skip_ws_forward(expression, end);
    let rhs_start = skip_ws_forward(expression, operator_start + assignment.operator.len());
    let rhs_end = process_expression_assignment_rhs_end(expression, rhs_start);
    let rhs = expression.get(rhs_start..rhs_end)?.trim();
    let locals = scopes
        .iter()
        .flat_map(|scope| scope.locals.iter().cloned())
        .collect::<Vec<_>>();
    let rewritten_rhs = rewrite_js_like_expression_with_locals(rhs, options, &locals);
    let replacement = match binding {
        "setup-ref" | "setup-maybe-ref" => {
            format!(
                "{ident}.value {} {}",
                assignment.operator,
                rewritten_rhs.trim()
            )
        }
        "setup-let" => {
            format!(
                "_isRef({ident}) ? {ident}.value {} {} : {ident} {} {}",
                assignment.operator,
                rewritten_rhs.trim(),
                assignment.operator,
                rewritten_rhs.trim()
            )
        }
        _ => return None,
    };
    Some((replacement, rhs_end))
}

pub(crate) fn rewrite_js_like_update(
    ident: &str,
    expression: &str,
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    arrow_local: bool,
) -> Option<(String, usize)> {
    if !options.inline || is_local(scopes, ident) || arrow_local {
        return None;
    }
    let binding = options.binding_metadata.get(ident).map(String::as_str)?;
    let update = process_expression_update_argument(expression, start, end)?;
    let (_, consumed_end) = process_expression_update_range(expression, start, end, update)?;
    let prefix = if update.prefix { update.operator } else { "" };
    let postfix = if update.prefix { "" } else { update.operator };
    let replacement = match binding {
        "setup-ref" | "setup-maybe-ref" => format!("{prefix}{ident}.value{postfix}"),
        "setup-let" => {
            format!("_isRef({ident}) ? {prefix}{ident}.value{postfix} : {prefix}{ident}{postfix}")
        }
        _ => return None,
    };
    Some((replacement, consumed_end))
}

pub(crate) fn rewrite_js_like_destructure_identifier(
    ident: &str,
    expression: &str,
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    arrow_param: bool,
) -> Option<(String, usize)> {
    if !options.inline
        || is_local(scopes, ident)
        || arrow_param
        || !process_expression_is_destructure_assignment(expression, start)
    {
        return None;
    }
    let binding = options.binding_metadata.get(ident).map(String::as_str)?;
    let rewritten = match binding {
        "setup-ref" | "setup-maybe-ref" => format!("{ident}.value"),
        "setup-let" => ident.to_string(),
        _ => return None,
    };
    if process_expression_object_shorthand(expression, start, end) {
        Some((format!("{ident}: {rewritten}"), end))
    } else {
        Some((rewritten, end))
    }
}

pub(crate) fn process_expression_update_range(
    expression: &str,
    start: usize,
    end: usize,
    update: ProcessExpressionUpdate,
) -> Option<(usize, usize)> {
    if update.prefix {
        let operator_start = previous_operator_start(expression, start, update.operator)?;
        Some((operator_start, end))
    } else {
        let operator_start = skip_ws_forward(expression, end);
        expression
            .get(operator_start..)
            .is_some_and(|tail| tail.starts_with(update.operator))
            .then_some((start, operator_start + update.operator.len()))
    }
}

pub(crate) fn previous_operator_start(
    expression: &str,
    start: usize,
    operator: &str,
) -> Option<usize> {
    let head = expression.get(..start)?.trim_end();
    if !head.ends_with(operator) {
        return None;
    }
    Some(head.len().saturating_sub(operator.len()))
}

pub(crate) fn read_identifier_at(source: &str, start: usize) -> Option<(&str, usize)> {
    let mut chars = source.get(start..)?.char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (relative, ch) in chars {
        if !is_identifier_continue(ch) {
            return Some((&source[start..end], end));
        }
        end = start + relative + ch.len_utf8();
    }
    Some((&source[start..end], end))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    pub(crate) locals: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclKind {
    Var,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier,
    Keyword,
    OpenParen,
    Comma,
    Dot,
    Other,
}

pub(crate) fn is_local(scopes: &[Scope], ident: &str) -> bool {
    scopes
        .iter()
        .rev()
        .any(|scope| scope.locals.iter().any(|local| local == ident))
}

pub(crate) fn next_non_ws(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

pub(crate) fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

pub(crate) fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

pub(crate) fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "if"
            | "else"
            | "for"
            | "in"
            | "of"
            | "try"
            | "catch"
            | "throw"
            | "new"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "async"
            | "await"
    )
}

pub(crate) fn is_global_or_literal(value: &str) -> bool {
    matches!(
        value,
        "true"
            | "false"
            | "null"
            | "undefined"
            | "this"
            | "Infinity"
            | "NaN"
            | "Math"
            | "Number"
            | "Date"
            | "Array"
            | "Object"
            | "Boolean"
            | "String"
            | "RegExp"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "JSON"
            | "Intl"
            | "BigInt"
            | "console"
            | "Error"
            | "TypeError"
            | "Symbol"
            | "Promise"
            | "Reflect"
            | "globalThis"
    )
}

pub(crate) fn is_generated_asset_import_ident(value: &str) -> bool {
    value
        .strip_prefix("_imports_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

pub(crate) fn rewrite_identifier(ident: &str, options: &Vue3CompilerOptions) -> String {
    match options.binding_metadata.get(ident).map(String::as_str) {
        Some("setup-ref") if options.inline => format!("{ident}.value"),
        Some("setup-maybe-ref") if options.inline => format!("_unref({ident})"),
        Some("setup-let") if options.inline => format!("_unref({ident})"),
        Some("setup-const" | "literal-const" | "setup-reactive-const") if options.inline => {
            ident.to_string()
        }
        Some("props") if options.inline => format!("__props.{ident}"),
        Some("props-aliased") if options.inline => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            render_props_access("__props", source)
        }
        Some("props-aliased") => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            render_props_access("$props", source)
        }
        Some("data" | "options") if options.inline => format!("_ctx.{ident}"),
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            format!("$setup.{ident}")
        }
        Some(kind) => format!("${kind}.{ident}"),
        None => format!("_ctx.{ident}"),
    }
}

pub(crate) fn render_props_access(base: &str, key: &str) -> String {
    if is_simple_identifier_ascii(key) {
        format!("{base}.{key}")
    } else {
        format!("{base}[{}]", quote_string(key))
    }
}

pub(crate) fn push_vue3_parser_diagnostic(
    ast: &mut Vue3Ast,
    code: Vue3ErrorCode,
    file_id: FileId,
    offset: usize,
) {
    let diagnostic = Vue3ParserDiagnostic {
        code: code.as_u16(),
        message: vue3_parse_error_message(code).into(),
        span: Some(Span::new(file_id, offset, offset)),
    };
    if let Some(root_node) = ast.root_node_mut() {
        if let Vue3AstKind::Root(root) = &mut root_node.kind {
            if !root.parser_diagnostics.iter().any(|existing| {
                existing.code == diagnostic.code && existing.span == diagnostic.span
            }) {
                root.parser_diagnostics.push(diagnostic);
            }
        }
    }
}

/// Returns Vue 3 parser diagnostics recorded on the AST root.
pub fn vue3_parser_diagnostics(ast: &Vue3Ast) -> Vec<Diagnostic> {
    ast.root_node()
        .and_then(|node| match &node.kind {
            Vue3AstKind::Root(root) => Some(root.parser_diagnostics.as_slice()),
            _ => None,
        })
        .into_iter()
        .flatten()
        .map(|diagnostic| {
            Diagnostic::error(diagnostic.code.to_string(), diagnostic.message.clone())
                .with_span(diagnostic.span)
        })
        .collect()
}

pub(crate) fn vue3_parse_error_message(code: Vue3ErrorCode) -> &'static str {
    match code {
        Vue3ErrorCode::DuplicateAttribute => "Duplicate attribute.",
        Vue3ErrorCode::EofBeforeTagName => "Unexpected EOF in tag.",
        Vue3ErrorCode::EofInTag => "Unexpected EOF in tag.",
        Vue3ErrorCode::MissingEndTagName => "End tag name was expected.",
        Vue3ErrorCode::XInvalidEndTag => "Invalid end tag.",
        Vue3ErrorCode::XMissingEndTag => "Element is missing end tag.",
        Vue3ErrorCode::XMissingInterpolationEnd => "Interpolation end sign was not found.",
        Vue3ErrorCode::XMissingDirectiveName => "Legal directive name was expected.",
        _ => "Vue compiler parse error",
    }
}

pub(crate) fn expression_diagnostics(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> Vec<Diagnostic> {
    let store = JsAstStore::new();
    let source_type = expression_source_type(options);
    let mut diagnostics = Vec::new();
    for node in &ast.nodes {
        match &node.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                push_expression_parse_diagnostic(
                    &store,
                    &interpolation.expression.source_string(),
                    node.span.source(),
                    source_type,
                    &mut diagnostics,
                );
            }
            Vue3AstKind::Element(element) => {
                for prop in &element.props {
                    if let Vue3Prop::Directive(dir) = prop {
                        push_directive_expression_diagnostic(
                            &store,
                            dir,
                            source_type,
                            &mut diagnostics,
                        );
                        if dir.name == "model" {
                            push_model_binding_diagnostic(element, dir, options, &mut diagnostics);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    diagnostics
}

pub(crate) fn push_directive_expression_diagnostic(
    store: &JsAstStore,
    dir: &Vue3Directive,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match dir.name.as_str() {
        "for" => push_for_expression_diagnostic(store, dir, source_type, diagnostics),
        "on" if dir.arg.is_some() => {
            if let Some(expression) = dir.exp.as_ref() {
                push_event_handler_parse_diagnostic(
                    store,
                    &expression.source_string(),
                    dir.exp_span,
                    source_type,
                    diagnostics,
                );
            }
        }
        _ => {
            if let Some(expression) = dir.exp.as_ref() {
                push_expression_parse_diagnostic(
                    store,
                    &expression.source_string(),
                    dir.exp_span,
                    source_type,
                    diagnostics,
                );
            }
        }
    }
}

pub(crate) fn push_for_expression_diagnostic(
    store: &JsAstStore,
    dir: &Vue3Directive,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expression) = dir.exp.as_ref() else {
        diagnostics.push(vue3_for_diagnostic(
            Vue3ErrorCode::XVForNoExpression,
            "v-for is missing expression.",
            dir.span,
        ));
        return;
    };
    let expression = expression.source_string();
    let expression = expression.trim();
    if expression.is_empty() {
        diagnostics.push(vue3_for_diagnostic(
            Vue3ErrorCode::XVForNoExpression,
            "v-for is missing expression.",
            dir.exp_span.or(dir.span),
        ));
        return;
    }
    if store.parse_for_expression(expression, source_type).is_err() {
        diagnostics.push(vue3_for_diagnostic(
            Vue3ErrorCode::XVForMalformedExpression,
            "v-for has invalid expression.",
            dir.exp_span.or(dir.span),
        ));
    }
}

pub(crate) fn vue3_for_diagnostic(
    code: Vue3ErrorCode,
    message: &str,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::vue3_error(code, message, span)
}

pub(crate) fn push_model_binding_diagnostic(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expression) = dir.exp.as_ref() else {
        diagnostics.push(vue3_model_diagnostic(
            "41",
            "v-model is missing expression.",
            dir.arg_span.or(dir.exp_span),
        ));
        return;
    };
    let raw = expression.source_string();
    let raw = raw.trim();
    if raw.is_empty() {
        diagnostics.push(vue3_model_diagnostic(
            "42",
            "v-model value must be a valid JavaScript member expression.",
            dir.exp_span,
        ));
        return;
    }
    match options.binding_metadata.get(raw).map(String::as_str) {
        Some("props" | "props-aliased") => diagnostics.push(vue3_model_diagnostic(
            "44",
            "v-model cannot be used on a prop, because local prop bindings are not writable.\nUse a v-bind binding combined with a v-on listener that emits update:x event instead.",
            dir.exp_span,
        )),
        Some("literal-const" | "setup-const") => diagnostics.push(vue3_model_diagnostic(
            "45",
            "v-model cannot be used on a const binding because it is not writable.",
            dir.exp_span,
        )),
        _ if model_binding_host_supports_expression_diagnostic(element)
            && !model_is_member_expression(raw) =>
        {
            diagnostics.push(vue3_model_diagnostic(
                "42",
                "v-model value must be a valid JavaScript member expression.",
                dir.exp_span,
            ));
        }
        _ => {}
    }
}

pub(crate) fn model_binding_host_supports_expression_diagnostic(element: &Vue3Element) -> bool {
    element.tag_type == Vue3ElementType::Component || vue3_dom_model_kind(element).is_some()
}

pub(crate) fn vue3_model_diagnostic(code: &str, message: &str, span: Option<Span>) -> Diagnostic {
    Diagnostic::error(code, message).with_span(span)
}

pub(crate) fn push_event_handler_parse_diagnostic(
    store: &JsAstStore,
    expression: &str,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = expression.trim();
    if expression.is_empty() {
        return;
    }
    let wrapped = format!("({expression})");
    if store.parse_expression(&wrapped, source_type).is_ok() {
        return;
    }
    if !expression.contains(';') {
        push_expression_parse_diagnostic(store, expression, span, source_type, diagnostics);
        return;
    }
    let parsed = store.parse_program(expression, source_type);
    if !parsed.panicked && parsed.errors.is_empty() {
        return;
    }
    diagnostics.push(js_program_errors_to_vue3_invalid_expression_diagnostic(
        &parsed.errors,
        expression,
        span,
    ));
}

pub(crate) fn push_expression_parse_diagnostic(
    store: &JsAstStore,
    expression: &str,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = expression.trim();
    if expression.is_empty() {
        return;
    }
    let wrapped = format!("({expression})");
    let Err(err) = store.parse_expression(&wrapped, source_type) else {
        return;
    };
    diagnostics.push(js_error_to_vue3_invalid_expression_diagnostic(
        &err, expression, span,
    ));
}

pub(crate) fn expression_source_type(options: &Vue3CompilerOptions) -> oxc_span::SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    }
}

pub(crate) fn quote_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

pub(crate) fn json_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
    {
        key.to_string()
    } else {
        quote_string(key)
    }
}

pub(crate) fn quote_text(value: &str) -> String {
    quote_string(value)
}

pub(crate) fn push_text_and_interpolations(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    token_start: usize,
    text: &str,
    options: &Vue3CompilerOptions,
) {
    let (open_delimiter, close_delimiter) = options
        .delimiters
        .as_ref()
        .map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    if open_delimiter.is_empty() || close_delimiter.is_empty() {
        push_text(ast, parent, file_id, token_start, text);
        return;
    }
    let mut cursor = 0usize;
    while let Some(open) = text[cursor..].find(open_delimiter) {
        let open = cursor + open;
        let expression_start = open + open_delimiter.len();
        let Some(close_offset) = text[expression_start..].find(close_delimiter) else {
            push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
            return;
        };
        if open > cursor {
            push_text(
                ast,
                parent,
                file_id,
                token_start + cursor,
                &text[cursor..open],
            );
        }
        let close = expression_start + close_offset;
        let expression = decode_html_text_entities(text[expression_start..close].trim());
        let _id = ast.push_child(
            parent,
            Vue3NodeKind::interpolation(expression),
            Some(Span::new(
                file_id,
                token_start + open,
                token_start + close + close_delimiter.len(),
            )),
        );
        cursor = close + close_delimiter.len();
    }
    if cursor < text.len() {
        push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
    }
}

pub(crate) fn push_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_html_text_entities(text);
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(&decoded);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decoded),
        Some(Span::new(file_id, start, start + text.len())),
    );
}

pub(crate) fn push_raw_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(text);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(text),
        Some(Span::new(file_id, start, start + text.len())),
    );
}
