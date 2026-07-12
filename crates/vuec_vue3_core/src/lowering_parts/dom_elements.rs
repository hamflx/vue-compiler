pub(crate) fn lower_vue3_element_to_dom_mir_kind(
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_kind: &HirNodeKind,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomMirKind {
    if element.tag_type == Vue3ElementType::SlotOutlet {
        let HirNodeKind::SlotOutlet(slot) = hir_kind else {
            return Vue3DomMirKind::Fragment;
        };
        return Vue3DomMirKind::RenderSlot(vuec_ast::Vue3RenderSlot {
            name: lower_hir_slot_outlet_name_to_dom_mir(slot),
            props: lower_vue3_slot_outlet_props_to_dom_mir(&slot.props, state),
            fallback: Vec::new(),
        });
    }

    let is_component = element.tag_type == Vue3ElementType::Component;
    let (mut props, directives, v_show) = lower_vue3_hir_payload_to_dom_mir(hir_kind, state);
    let content = inject_vue3_dom_content_props(&mut props, element, ast_node, state);
    let models = inject_vue3_dom_model_props(&mut props, element, ast_node, state);
    inject_vue3_transition_persisted_prop(&mut props, element, ast, ast_node);
    let tag = lower_vue3_element_tag_to_dom_mir(element, &props);
    Vue3DomMirKind::VNodeCall(Vue3VNodeCall {
        tag,
        props,
        v_show,
        directives,
        models,
        content,
        children: if ast_node.children.is_empty() {
            MirChildren::None
        } else {
            MirChildren::Nodes(Vec::new())
        },
        patch_flag: Vue3PatchFlags {
            bits: vue3_dom_mir_patch_flag(ast, ast_id, element, &state.options),
        },
        dynamic_props: vue3_dom_mir_dynamic_props(element),
        is_block: false,
        disable_tracking: false,
        is_component,
    })
}

pub(crate) fn inject_vue3_dom_content_props(
    props: &mut Vue3DomProps,
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3DomContent> {
    let mut segment_index = 0usize;
    let mut content = None;
    let mut segments = Vec::new();
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(_) => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(dir) if dir.name == "html" || dir.name == "text" => {
                let lowered = if dir.name == "html" {
                    Vue3DomContent::Html {
                        expression: lower_vue3_dom_content_expression(dir, ast_node, state),
                    }
                } else {
                    Vue3DomContent::Text {
                        expression: lower_vue3_dom_content_expression(dir, ast_node, state),
                    }
                };
                if content.is_none() {
                    content = Some(lowered.clone());
                }
                segments.push(Vue3DomPropSegment::Content(lowered));
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" || dir.name == "on" => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(_) => {}
        }
    }
    while segment_index < props.segments.len() {
        push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
    }
    if content.is_some() {
        props.segments = segments;
    }
    content
}

pub(crate) fn inject_vue3_dom_model_props(
    props: &mut Vue3DomProps,
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Vec<Vue3DomModel> {
    let mut segment_index = 0usize;
    let mut models = Vec::new();
    let mut segments = Vec::new();
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(_) => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(dir) if dir.name == "model" => {
                if let Some(model) = lower_vue3_dom_model(element, dir, ast_node, state) {
                    models.push(model.clone());
                    segments.push(Vue3DomPropSegment::Model(model));
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" || dir.name == "on" => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(_) => {}
        }
    }
    while segment_index < props.segments.len() {
        push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
    }
    if !models.is_empty() {
        props.segments = segments;
    }
    models
}

pub(crate) fn lower_vue3_dom_model(
    element: &Vue3Element,
    directive: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3DomModel> {
    if element.tag_type == Vue3ElementType::Component || directive.arg.is_some() {
        return None;
    }
    if !matches!(element.tag.as_str(), "input" | "textarea" | "select")
        && !state
            .options
            .custom_elements
            .iter()
            .any(|candidate| candidate == &element.tag)
    {
        return None;
    }
    let expression = directive.exp.as_ref()?;
    if expression.source_string().trim().is_empty() {
        return None;
    }
    let kind = vue3_dom_model_kind(element)?;
    Some(Vue3DomModel {
        expression: register_or_reuse_vue3_expression_with_span(
            &mut state.js,
            expression,
            directive.exp_span.or_else(|| ast_node.span.source()),
            state.source_type,
        ),
        kind,
        modifiers: directive.modifiers.clone(),
    })
}

pub(crate) fn vue3_dom_model_kind(element: &Vue3Element) -> Option<Vue3DomModelKind> {
    match element.tag.as_str() {
        "select" => Some(Vue3DomModelKind::Select),
        "textarea" => Some(Vue3DomModelKind::Text),
        "input" => vue3_dom_input_model_kind(element),
        _ if element.tag_type == Vue3ElementType::Element => Some(Vue3DomModelKind::Text),
        _ => None,
    }
}

pub(crate) fn vue3_dom_input_model_kind(element: &Vue3Element) -> Option<Vue3DomModelKind> {
    if vue3_dom_has_dynamic_key_v_bind(element) {
        return Some(Vue3DomModelKind::Dynamic);
    }
    match vue3_dom_input_type(element) {
        Some(Vue3DomInputType::Dynamic) => Some(Vue3DomModelKind::Dynamic),
        Some(Vue3DomInputType::Static(value)) => match value.as_str() {
            "radio" => Some(Vue3DomModelKind::Radio),
            "checkbox" => Some(Vue3DomModelKind::Checkbox),
            "file" => None,
            _ => Some(Vue3DomModelKind::Text),
        },
        None => Some(Vue3DomModelKind::Text),
    }
}

pub(crate) enum Vue3DomInputType {
    Static(String),
    Dynamic,
}

pub(crate) fn vue3_dom_input_type(element: &Vue3Element) -> Option<Vue3DomInputType> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "type" => Some(Vue3DomInputType::Static(
            attr.value.clone().unwrap_or_default(),
        )),
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "type") =>
        {
            Some(Vue3DomInputType::Dynamic)
        }
        _ => None,
    })
}

pub(crate) fn vue3_dom_has_dynamic_key_v_bind(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind" && dir.exp.is_some() && (dir.arg.is_none() || dir.is_dynamic_arg)
        )
    })
}

pub(crate) fn push_existing_vue3_dom_prop_segment(
    props: &Vue3DomProps,
    segments: &mut Vec<Vue3DomPropSegment>,
    index: &mut usize,
) {
    if let Some(segment) = props.segments.get(*index).cloned() {
        segments.push(segment);
        *index += 1;
    }
}

pub(crate) fn lower_vue3_dom_content_expression(
    directive: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<JsExprId> {
    directive.exp.as_ref().and_then(|expression| {
        (!expression.source_string().trim().is_empty()).then(|| {
            register_or_reuse_vue3_expression_with_span(
                &mut state.js,
                expression,
                directive.exp_span.or_else(|| ast_node.span.source()),
                state.source_type,
            )
        })
    })
}

pub(crate) fn lower_hir_slot_outlet_name_to_dom_mir(slot: &HirSlotOutlet) -> Vue3DomSlotName {
    if let Some(binding) = slot
        .props
        .dynamic_bindings
        .iter()
        .find(|binding| !binding.dynamic_arg && binding.name == "name")
    {
        Vue3DomSlotName::Dynamic(binding.value)
    } else {
        Vue3DomSlotName::Static(slot.name.clone().unwrap_or_else(|| "default".into()))
    }
}

pub(crate) fn lower_vue3_slot_outlet_props_to_dom_mir(
    props: &HirProps,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomProps {
    let filtered = filter_vue3_slot_outlet_name_props(props);
    lower_hir_props_to_dom_mir(&filtered, false, state)
}

pub(crate) fn filter_vue3_slot_outlet_name_props(props: &HirProps) -> HirProps {
    let mut filtered = props.clone();
    filtered.static_attrs.retain(|attr| attr.name != "name");
    filtered
        .dynamic_bindings
        .retain(|binding| binding.dynamic_arg || binding.name != "name");
    filtered.segments.retain(|segment| match segment {
        HirPropSegment::StaticAttr(attr) => attr.name != "name",
        HirPropSegment::DynamicBinding(binding) => binding.dynamic_arg || binding.name != "name",
        HirPropSegment::Event(_)
        | HirPropSegment::ObjectBinding(_)
        | HirPropSegment::ObjectListeners(_) => true,
    });
    filtered
}

pub(crate) fn lower_vue3_element_tag_to_dom_mir(
    element: &Vue3Element,
    props: &Vue3DomProps,
) -> Vue3DomTag {
    if element.tag_type != Vue3ElementType::Component {
        return Vue3DomTag::Native(element.tag.clone());
    }
    if let Some(expression) = props
        .dynamic_bindings
        .iter()
        .find(|binding| !binding.dynamic_arg && binding.name == "is")
        .map(|binding| binding.value)
    {
        return Vue3DomTag::DynamicComponent(expression);
    }
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        return Vue3DomTag::RuntimeHelper(helper);
    }
    Vue3DomTag::ComponentAsset(element.tag.clone())
}

pub(crate) fn inject_vue3_transition_persisted_prop(
    props: &mut Vue3DomProps,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
) {
    if element.tag_type != Vue3ElementType::Component
        || vue3_core_component_runtime_helper(&element.tag) != Some(RuntimeHelper::Vue3Transition)
        || !vue3_transition_single_child_has_v_show(ast, &ast_node.children)
        || vue3_dom_final_prop_group_has_static_key(props, "persisted")
    {
        return;
    }
    let attr = Vue3DomStaticAttr {
        name: "persisted".into(),
        value: String::new(),
    };
    props.static_attrs.push(attr.clone());
    if !props.segments.is_empty() {
        props.segments.push(Vue3DomPropSegment::StaticAttr(attr));
    }
}

pub(crate) fn vue3_dom_final_prop_group_has_static_key(props: &Vue3DomProps, name: &str) -> bool {
    if props.segments.is_empty() {
        return props.static_attrs.iter().any(|attr| attr.name == name)
            || props
                .dynamic_bindings
                .iter()
                .any(|binding| !binding.dynamic_arg && binding.name == name);
    }
    for segment in props.segments.iter().rev() {
        match segment {
            Vue3DomPropSegment::StaticAttr(attr) if attr.name == name => return true,
            Vue3DomPropSegment::DynamicBinding(binding)
                if !binding.dynamic_arg && binding.name == name =>
            {
                return true;
            }
            Vue3DomPropSegment::ObjectBinding(_) | Vue3DomPropSegment::ObjectListeners(_) => {
                return false;
            }
            Vue3DomPropSegment::StaticAttr(_)
            | Vue3DomPropSegment::DynamicBinding(_)
            | Vue3DomPropSegment::Content(_)
            | Vue3DomPropSegment::Model(_)
            | Vue3DomPropSegment::Event(_) => {}
        }
    }
    false
}

pub(crate) fn vue3_transition_single_child_has_v_show(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    let visible = vue3_transition_visible_child_ids(ast, children);
    let [child_id] = visible.as_slice() else {
        return false;
    };
    let Some(child) = ast.node(*child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    directive_by_name(element, "show").is_some()
        && directive_by_name(element, "if").is_none()
        && directive_by_name(element, "else").is_none()
        && directive_by_name(element, "else-if").is_none()
        && directive_by_name(element, "for").is_none()
}

pub(crate) fn vue3_transition_visible_child_ids(ast: &Vue3Ast, children: &[NodeId]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|child_id| {
            ast.node(*child_id).is_some_and(|child| match &child.kind {
                Vue3AstKind::Comment(_) => false,
                Vue3AstKind::Text(text) => !text.value.chars().all(is_vue3_html_whitespace),
                _ => true,
            })
        })
        .collect()
}

pub(crate) fn vue3_dom_mir_patch_flag(
    ast: &Vue3Ast,
    ast_id: NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
) -> i32 {
    let children = ast
        .node(ast_id)
        .map(|node| node.children.as_slice())
        .unwrap_or(&[]);
    let mut bits = 0;
    if has_dynamic_arg_binding(element) {
        bits |= 16;
    } else {
        if has_class_binding(element) && element.tag_type != Vue3ElementType::Component {
            bits |= 2;
        }
        if has_style_binding(element) && element.tag_type != Vue3ElementType::Component {
            bits |= 4;
        }
        if !vue3_dom_mir_props_patch_names(element).is_empty() {
            bits |= 8;
        }
        if has_hydration_event_binding(element) || has_prop_bind_modifier(element) {
            bits |= 32;
        }
    }
    if element.tag != "template" && child_sequence_is_direct_dynamic_text(ast, children, options) {
        bits |= 1;
    }
    if (bits == 0 || bits == 32)
        && (has_vnode_hook(element)
            || has_runtime_directive(element)
            || has_native_v_model(element))
    {
        bits |= 512;
    }
    if element.tag_type == Vue3ElementType::Component && component_has_dynamic_slots(ast, children)
    {
        bits |= 1024;
    }
    bits
}

pub(crate) fn component_has_dynamic_slots(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    visible_child_ids(ast, children).iter().any(|child_id| {
        let Some(child) = ast.node(*child_id) else {
            return false;
        };
        let Vue3AstKind::Element(element) = &child.kind else {
            return false;
        };
        directive_by_name(element, "slot").is_some_and(|slot| slot.is_dynamic_arg)
            || directive_by_name(element, "slot").is_some()
                && (directive_by_name(element, "if").is_some()
                    || directive_by_name(element, "else").is_some()
                    || directive_by_name(element, "else-if").is_some()
                    || directive_by_name(element, "for").is_some())
    })
}

pub(crate) fn lower_vue3_hir_payload_to_dom_mir(
    hir_kind: &HirNodeKind,
    state: &mut Vue3DomLoweringState,
) -> (Vue3DomProps, Vec<Vue3DomDirective>, Option<JsExprId>) {
    match hir_kind {
        HirNodeKind::Element(element) => {
            let mut v_show = None;
            let directives = element
                .directives
                .iter()
                .filter_map(|directive| {
                    if directive.name == "show" {
                        v_show = directive.expression;
                        None
                    } else if vue3_directive_needs_runtime_asset(&directive.name) {
                        Some(lower_hir_directive_to_dom_mir(directive))
                    } else {
                        None
                    }
                })
                .collect();
            (
                lower_hir_props_to_dom_mir(&element.props, false, state),
                directives,
                v_show,
            )
        }
        HirNodeKind::Component(component) => (
            lower_hir_props_to_dom_mir(&component.props, true, state),
            Vec::new(),
            None,
        ),
        HirNodeKind::SlotOutlet(slot) => (
            lower_hir_props_to_dom_mir(&slot.props, false, state),
            Vec::new(),
            None,
        ),
        _ => (Vue3DomProps::default(), Vec::new(), None),
    }
}

pub(crate) fn lower_hir_props_to_dom_mir(
    props: &HirProps,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomProps {
    if !props.segments.is_empty() {
        return lower_ordered_hir_props_to_dom_mir(props, is_component, state);
    }

    Vue3DomProps {
        injected_key: None,
        static_attrs: props
            .static_attrs
            .iter()
            .map(lower_hir_static_attr_to_dom_mir)
            .collect(),
        dynamic_bindings: props
            .dynamic_bindings
            .iter()
            .map(lower_hir_binding_to_dom_mir)
            .collect(),
        events: props
            .events
            .iter()
            .map(|event| lower_hir_event_to_dom_mir(event, is_component, state))
            .collect(),
        object_bindings: props
            .object_bindings
            .iter()
            .map(|binding| Vue3DomObjectBinding {
                value: binding.value,
            })
            .collect(),
        object_listeners: props
            .object_listeners
            .iter()
            .map(|listeners| Vue3DomObjectListeners {
                value: listeners.value,
                preserve_case: !is_component,
            })
            .collect(),
        normalize: Vue3DomPropsNormalize {
            normalize_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
            guard_reactive_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
        },
        segments: Vec::new(),
    }
}

pub(crate) fn lower_ordered_hir_props_to_dom_mir(
    props: &HirProps,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomProps {
    let mut segments = Vec::new();
    let mut static_attrs = Vec::new();
    let mut dynamic_bindings = Vec::new();
    let mut events = Vec::new();
    let mut object_bindings = Vec::new();
    let mut object_listeners = Vec::new();

    for segment in &props.segments {
        match segment {
            HirPropSegment::StaticAttr(attr) => {
                let lowered = lower_hir_static_attr_to_dom_mir(attr);
                static_attrs.push(lowered.clone());
                segments.push(Vue3DomPropSegment::StaticAttr(lowered));
            }
            HirPropSegment::DynamicBinding(binding) => {
                let lowered = lower_hir_binding_to_dom_mir(binding);
                dynamic_bindings.push(lowered.clone());
                segments.push(Vue3DomPropSegment::DynamicBinding(lowered));
            }
            HirPropSegment::Event(event) => {
                let lowered = lower_hir_event_to_dom_mir(event, is_component, state);
                events.push(lowered.clone());
                segments.push(Vue3DomPropSegment::Event(lowered));
            }
            HirPropSegment::ObjectBinding(binding) => {
                let lowered = Vue3DomObjectBinding {
                    value: binding.value,
                };
                object_bindings.push(lowered.clone());
                segments.push(Vue3DomPropSegment::ObjectBinding(lowered));
            }
            HirPropSegment::ObjectListeners(listeners) => {
                let lowered = Vue3DomObjectListeners {
                    value: listeners.value,
                    preserve_case: !is_component,
                };
                object_listeners.push(lowered.clone());
                segments.push(Vue3DomPropSegment::ObjectListeners(lowered));
            }
        }
    }

    Vue3DomProps {
        injected_key: None,
        segments,
        static_attrs,
        dynamic_bindings,
        events,
        object_bindings,
        object_listeners,
        normalize: Vue3DomPropsNormalize {
            normalize_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
            guard_reactive_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
        },
    }
}

pub(crate) fn lower_hir_static_attr_to_dom_mir(attr: &HirStaticAttr) -> Vue3DomStaticAttr {
    Vue3DomStaticAttr {
        name: attr.name.clone(),
        value: attr.value.clone(),
    }
}

pub(crate) fn lower_hir_binding_to_dom_mir(binding: &HirBinding) -> Vue3DomBinding {
    Vue3DomBinding {
        name: binding.name.clone(),
        dynamic_name: binding.dynamic_name,
        value: binding.value,
        dynamic_arg: binding.dynamic_arg,
        camel: binding.modifiers.iter().any(|modifier| modifier == "camel"),
        force_prop: binding.modifiers.iter().any(|modifier| modifier == "prop"),
        force_attr: binding.modifiers.iter().any(|modifier| modifier == "attr"),
    }
}

pub(crate) fn lower_hir_event_to_dom_mir(
    event: &HirEvent,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomEvent {
    let cache = vue3_dom_event_cache(event, is_component, state);
    let base_name = if event.dynamic_arg {
        event.name.clone()
    } else if is_component {
        event_handler_prop_name_for_component(&event.name)
    } else {
        event_handler_prop_name_for_element(&event.name)
    };
    let modifiers = vue3_dom_event_modifiers_for(&base_name, event.dynamic_arg, &event.modifiers);
    Vue3DomEvent {
        name: if event.dynamic_arg {
            event.name.clone()
        } else if is_component {
            let event_name = modifiers
                .click_event
                .map(vue3_dom_click_event_name)
                .unwrap_or_else(|| event.name.clone());
            event_handler_prop_name_for_component(&event_name)
        } else {
            let event_name = modifiers
                .click_event
                .map(vue3_dom_click_event_name)
                .unwrap_or_else(|| event.name.clone());
            event_handler_prop_name_for_element(&event_name)
        },
        dynamic_name: event.dynamic_name,
        handler: event.handler,
        dynamic_arg: event.dynamic_arg,
        runtime_modifiers: modifiers.runtime_modifiers,
        key_modifiers: modifiers.key_modifiers,
        option_modifiers: modifiers.option_modifiers,
        click_event: modifiers.click_event,
        cache,
    }
}

#[derive(Default)]
pub(crate) struct Vue3DomEventModifiers {
    pub(crate) runtime_modifiers: Vec<String>,
    pub(crate) key_modifiers: Vec<String>,
    pub(crate) option_modifiers: Vec<String>,
    pub(crate) click_event: Option<Vue3DomClickEvent>,
}

pub(crate) fn vue3_dom_event_modifiers_for(
    event_key: &str,
    dynamic_arg: bool,
    raw_modifiers: &[String],
) -> Vue3DomEventModifiers {
    let mut modifiers = Vue3DomEventModifiers::default();
    for modifier in raw_modifiers {
        if vue3_dom_event_option_modifier(modifier) {
            modifiers.option_modifiers.push(modifier.clone());
            continue;
        }
        if vue3_dom_event_maybe_key_modifier(modifier) {
            if dynamic_arg {
                modifiers.runtime_modifiers.push(modifier.clone());
                modifiers.key_modifiers.push(modifier.clone());
            } else if vue3_dom_event_is_keyboard_event_key(event_key) {
                modifiers.key_modifiers.push(modifier.clone());
            } else {
                modifiers.runtime_modifiers.push(modifier.clone());
            }
            continue;
        }
        if vue3_dom_event_non_key_modifier(modifier) {
            modifiers.runtime_modifiers.push(modifier.clone());
        } else if dynamic_arg || vue3_dom_event_is_keyboard_event_key(event_key) {
            modifiers.key_modifiers.push(modifier.clone());
        }
    }
    if dynamic_arg || event_key.eq_ignore_ascii_case("onclick") {
        if modifiers
            .runtime_modifiers
            .iter()
            .any(|modifier| modifier == "right")
        {
            modifiers.click_event = Some(Vue3DomClickEvent::ContextMenu);
        }
        if modifiers
            .runtime_modifiers
            .iter()
            .any(|modifier| modifier == "middle")
        {
            modifiers.click_event = Some(Vue3DomClickEvent::MouseUp);
        }
    }
    modifiers
}

pub(crate) fn vue3_dom_event_option_modifier(modifier: &str) -> bool {
    matches!(modifier, "passive" | "once" | "capture")
}

pub(crate) fn vue3_dom_event_non_key_modifier(modifier: &str) -> bool {
    matches!(
        modifier,
        "stop" | "prevent" | "self" | "ctrl" | "shift" | "alt" | "meta" | "exact" | "middle"
    )
}

pub(crate) fn vue3_dom_event_maybe_key_modifier(modifier: &str) -> bool {
    matches!(modifier, "left" | "right")
}

pub(crate) fn vue3_dom_event_is_keyboard_event_key(event_key: &str) -> bool {
    matches!(
        event_key.to_ascii_lowercase().as_str(),
        "onkeyup" | "onkeydown" | "onkeypress"
    )
}

pub(crate) fn vue3_dom_click_event_name(click_event: Vue3DomClickEvent) -> String {
    match click_event {
        Vue3DomClickEvent::ContextMenu => "contextmenu".into(),
        Vue3DomClickEvent::MouseUp => "mouseup".into(),
    }
}

pub(crate) fn vue3_dom_event_cache(
    event: &HirEvent,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3DomEventCache> {
    if !state.options.cache_handlers || state.in_v_once > 0 || is_component {
        return None;
    }
    if event.dynamic_arg {
        return None;
    }
    let index = state.next_cache_index;
    state.next_cache_index += 1;
    Some(Vue3DomEventCache { index })
}

pub(crate) fn lower_hir_directive_to_dom_mir(directive: &HirDirectiveUse) -> Vue3DomDirective {
    Vue3DomDirective {
        name: directive.name.clone(),
        argument: directive.argument.clone(),
        dynamic_argument: directive.dynamic_argument,
        expression: directive.expression,
        modifiers: directive.modifiers.clone(),
    }
}

pub(crate) fn vue3_dom_mir_dynamic_props(element: &Vue3Element) -> Vec<String> {
    if has_dynamic_arg_binding(element) {
        return Vec::new();
    }
    let mut props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" && !event_directive_is_vnode_hook(dir) => {
                if dir.is_dynamic_arg || dir.arg.is_none() {
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
                if is_asset_import_binding(dir) {
                    return None;
                }
                if dir.is_dynamic_arg {
                    return None;
                }
                vue3_bind_directive_static_dom_key(dir, true)
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && vue3_dom_model_kind(element).is_some() =>
            {
                Some("onUpdate:modelValue".into())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    props.extend(vue3_dom_content_dynamic_props(element));
    props
}

pub(crate) fn vue3_dom_mir_props_patch_names(element: &Vue3Element) -> Vec<String> {
    let mut props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" && !event_directive_is_vnode_hook(dir) => {
                if dir.is_dynamic_arg || dir.arg.is_none() {
                    return None;
                }
                let event = dir
                    .arg
                    .as_ref()
                    .map(Vue3Expression::source_string)
                    .unwrap_or_default();
                Some(event_handler_prop_name(element, &event))
            }
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && !has_class_bind_dir(dir)
                    && !has_style_bind_dir(dir)
                    && !has_key_bind_dir(dir) =>
            {
                if is_asset_import_binding(dir) {
                    return None;
                }
                if dir.is_dynamic_arg {
                    return None;
                }
                vue3_bind_directive_static_dom_key(dir, true)
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && vue3_dom_model_kind(element).is_some() =>
            {
                Some("onUpdate:modelValue".into())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    props.extend(vue3_dom_content_dynamic_props(element));
    props
}

pub(crate) fn vue3_dom_content_dynamic_props(element: &Vue3Element) -> Vec<String> {
    element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "html" => Some("innerHTML".into()),
            Vue3Prop::Directive(dir) if dir.name == "text" => Some("textContent".into()),
            _ => None,
        })
        .collect()
}

pub(crate) fn has_native_v_model(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "model"
                    && dir.arg.is_none()
                    && vue3_dom_model_kind(element).is_some()
        )
    })
}

pub(crate) fn vue3_bind_directive_static_dom_key(
    directive: &Vue3Directive,
    apply_dom_prefix: bool,
) -> Option<String> {
    if directive.is_dynamic_arg {
        return None;
    }
    let name = directive.arg.as_ref().map(Vue3Expression::source_string)?;
    let binding = Vue3DomBinding {
        name,
        dynamic_name: None,
        value: JsExprId(0),
        dynamic_arg: false,
        camel: directive
            .modifiers
            .iter()
            .any(|modifier| modifier == "camel"),
        force_prop: directive
            .modifiers
            .iter()
            .any(|modifier| modifier == "prop"),
        force_attr: directive
            .modifiers
            .iter()
            .any(|modifier| modifier == "attr"),
    };
    Some(render_vue3_dom_binding_static_key(
        &binding,
        apply_dom_prefix,
    ))
}

pub(crate) fn has_style_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "style")
        )
    })
}

pub(crate) fn has_style_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "style")
}

pub(crate) fn has_dynamic_arg_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if matches!(dir.name.as_str(), "bind" | "on")
                    && (dir.is_dynamic_arg || dir.arg.is_none())
        )
    })
}

pub(crate) fn has_prop_bind_modifier(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind" && dir.modifiers.iter().any(|modifier| modifier == "prop")
        )
    })
}

pub(crate) fn has_hydration_event_binding(element: &Vue3Element) -> bool {
    let is_component = element.tag_type == Vue3ElementType::Component;
    element.props.iter().any(|prop| {
        let Vue3Prop::Directive(dir) = prop else {
            return false;
        };
        if dir.name != "on" || event_directive_is_vnode_hook(dir) || is_component {
            return false;
        }
        let Some(event) = dir.arg.as_ref().map(Vue3Expression::source_string) else {
            return false;
        };
        let prop = event_handler_prop_name(element, &event);
        !prop.eq_ignore_ascii_case("onclick") && prop != "onUpdate:modelValue"
    })
}

pub(crate) fn lower_vue3_element_control_flow_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<Option<(NodeId, NodeId)>> {
    let if_dir = directive_by_name(element, "if")
        .or_else(|| directive_by_name(element, "else-if"))
        .filter(|dir| dir.exp.is_some());
    if let Some(if_dir) = if_dir {
        let lowered = lower_vue3_with_once_cache(
            element,
            ast_node,
            mir_parent,
            state,
            |wrapper_id, state| {
                lower_vue3_if_directive_to_dom_mir(
                    ast_id, element, ast, ast_node, if_dir, hir_parent, wrapper_id, state,
                )
            },
        );
        return Some(lowered);
    }
    if let Some(for_dir) = directive_by_name(element, "for") {
        let lowered = lower_vue3_with_once_cache(
            element,
            ast_node,
            mir_parent,
            state,
            |wrapper_id, state| {
                lower_vue3_for_directive_to_dom_mir(
                    ast_id, element, ast, ast_node, for_dir, None, hir_parent, wrapper_id, state,
                )
            },
        );
        return Some(lowered);
    }
    None
}

fn vue3_dom_is_structural_template(element: &Vue3Element) -> bool {
    element.tag_type == Vue3ElementType::Template
        && directive_by_name(element, "slot").is_none()
}

pub(crate) fn lower_vue3_if_branch_chain_to_dom_mir(
    branch_ids: &[NodeId],
    branch_key_base: usize,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    let first_id = *branch_ids.first()?;
    let first_node = ast.node(first_id)?;
    let first_element = match &first_node.kind {
        Vue3AstKind::Element(element) => element,
        _ => return None,
    };
    lower_vue3_with_once_cache(
        first_element,
        first_node,
        mir_parent,
        state,
        |wrapper_id, state| {
            lower_vue3_if_branch_chain_inner_to_dom_mir(
                branch_ids,
                branch_key_base,
                ast,
                hir_parent,
                wrapper_id,
                state,
            )
        },
    )
}

fn lower_vue3_if_branch_chain_inner_to_dom_mir(
    branch_ids: &[NodeId],
    branch_key_base: usize,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    let first_id = *branch_ids.first()?;
    let first_node = ast.node(first_id)?;
    let first_element = match &first_node.kind {
        Vue3AstKind::Element(element) => element,
        _ => return None,
    };
    let first_dir = directive_by_name(first_element, "if")
        .or_else(|| directive_by_name(first_element, "else-if"))?;
    let condition =
        lower_vue3_optional_condition(first_dir, first_node, &mut state.js, state.source_type);
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::If(HirIf {
            branches: Vec::new(),
        }),
        first_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3DomMirKind::If { condition },
        first_node.span.clone(),
    );
    state.map.record_ast_to_hir(first_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let mut previous_branch_mir = mir_id;
    for (branch_offset, branch_id) in branch_ids.iter().enumerate() {
        let branch_key = branch_key_base
            .checked_add(branch_offset)
            .and_then(|key| u32::try_from(key).ok());
        let Some(branch_node) = ast.node(*branch_id) else {
            continue;
        };
        let Vue3AstKind::Element(branch_element) = &branch_node.kind else {
            continue;
        };
        let condition = if *branch_id == first_id {
            condition
        } else {
            vue3_branch_condition(
                branch_element,
                branch_node,
                &mut state.js,
                state.source_type,
            )
        };
        let branch_mir = if *branch_id != first_id {
            let branch_mir = state.mir.push_child(
                previous_branch_mir,
                Vue3DomMirKind::If { condition },
                branch_node.span.clone(),
            );
            state.map.record_ast_to_hir(*branch_id, hir_id);
            state.map.record_hir_to_mir(hir_id, branch_mir);
            branch_mir
        } else {
            mir_id
        };
        let suppress_branch_once = *branch_id != first_id
            && directive_by_name(branch_element, "once").is_some();
        state.in_v_once += u32::from(suppress_branch_once);
        let body = if let Some(for_dir) = directive_by_name(branch_element, "for") {
            lower_vue3_for_directive_to_dom_mir(
                *branch_id,
                branch_element,
                ast,
                branch_node,
                for_dir,
                branch_key,
                hir_id,
                branch_mir,
                state,
            )
        } else if vue3_dom_is_structural_template(branch_element) {
            lower_vue3_structural_template_body_to_dom_mir(
                branch_node,
                ast,
                hir_id,
                branch_mir,
                branch_key.map(Vue3DomKey::Branch),
                Vue3StructuralTemplateBodyKind::If,
                state,
            )
        } else {
            lower_vue3_non_control_element_to_dom_mir(
                *branch_id,
                branch_element,
                ast,
                branch_node,
                hir_id,
                branch_mir,
                state,
            )
        };
        state.in_v_once -= u32::from(suppress_branch_once);
        if let Some((body_hir, body_mir)) = body {
            if let Some(branch_key) = branch_key {
                inject_vue3_dom_key(body_mir, Vue3DomKey::Branch(branch_key), state);
            }
            if let Some(node) = state.hir.node_mut(hir_id) {
                if let HirNodeKind::If(hir_if) = &mut node.kind {
                    hir_if.branches.push(HirIfBranch {
                        condition,
                        body: body_hir,
                    });
                }
            }
        }
        previous_branch_mir = branch_mir;
    }

    Some((hir_id, mir_id))
}

pub(crate) fn lower_vue3_for_directive_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    directive: &Vue3Directive,
    branch_key: Option<u32>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    let parsed = lower_vue3_for_directive(directive, ast_node, &mut state.js, state.source_type)?;
    let is_stable = vue3_dom_for_source_is_stable(parsed.source, state);
    let has_key = vue3_for_has_key_prop(element);
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::For(HirFor {
            source: parsed.source,
            value_alias: parsed.value_alias,
            key_alias: parsed.key_alias,
            index_alias: parsed.index_alias,
            body: NodeId(0),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3DomMirKind::For(Vue3ForMir {
            source: parsed.source,
            is_stable,
            has_key,
            value_alias: parsed.value_alias,
            key_alias: parsed.key_alias,
            index_alias: parsed.index_alias,
            key: None,
            branch_key,
            memo: None,
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let is_structural_template = vue3_dom_is_structural_template(element);
    let mut key = is_structural_template
        .then(|| vue3_for_key_mir_expr(element, ast_node, &mut state.js, state.source_type))
        .flatten();
    let body = if is_structural_template {
        lower_vue3_structural_template_body_to_dom_mir(
            ast_node,
            ast,
            hir_id,
            mir_id,
            key.clone().map(Vue3DomKey::Value),
            Vue3StructuralTemplateBodyKind::For,
            state,
        )
    } else {
        lower_vue3_non_control_element_to_dom_mir(
            ast_id, element, ast, ast_node, hir_id, mir_id, state,
        )
    };
    if !is_structural_template {
        key = vue3_for_key_mir_expr(element, ast_node, &mut state.js, state.source_type);
    }
    if let Some((body_hir, body_mir)) = body {
        let can_use_vnode = is_stable
            && (is_structural_template || directive_by_name(element, "memo").is_none());
        set_vue3_dom_for_body_block(body_mir, can_use_vnode, state);
        if let Some(node) = state.hir.node_mut(hir_id) {
            if let HirNodeKind::For(hir_for) = &mut node.kind {
                hir_for.body = body_hir;
            }
        }
    }
    let memo = vue3_for_memo_mir(element, ast_node, state);
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue3DomMirKind::For(for_mir) = &mut node.kind {
            for_mir.key = key;
            for_mir.memo = memo;
        }
    }
    Some((hir_id, mir_id))
}

fn vue3_for_has_key_prop(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| match prop {
        Vue3Prop::Directive(dir) => {
            dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "key")
        }
        Vue3Prop::Attribute(attr) => attr.name == "key",
    })
}

fn vue3_dom_for_source_is_stable(source: JsExprId, state: &Vue3DomLoweringState) -> bool {
    let Some(source) = state.js.expressions().get(source.0 as usize) else {
        return false;
    };
    let source = source.source.trim();
    static_const_eval_source(source).is_some()
        || (uses_prefixed_identifiers(&state.options)
            && process_expression_is_const_binding(source, &state.options))
}

pub(crate) fn vue3_for_key_mir_expr(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> Option<MirExpr> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "key") =>
        {
            dir.exp
                .as_ref()
                .filter(|exp| !exp.source_string().trim().is_empty())
                .map(|exp| {
                    MirExpr::JsExpr(register_or_reuse_vue3_expression_with_span(
                        js,
                        exp,
                        dir.exp_span.or_else(|| ast_node.span.source()),
                        source_type,
                    ))
                })
        }
        Vue3Prop::Attribute(attr) if attr.name == "key" => attr.value.clone().map(MirExpr::String),
        _ => None,
    })
}

pub(crate) fn vue3_for_memo_mir(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3ForMemo> {
    let memo = directive_by_name(element, "memo")?;
    let expression = register_vue3_expression_with_span(
        &mut state.js,
        memo.exp
            .as_ref()
            .unwrap_or(&Vue3Expression::Raw(String::new())),
        memo.exp_span.or_else(|| ast_node.span.source()),
        state.source_type,
    );
    let index = state.next_cache_index;
    state.next_cache_index += 1;
    Some(Vue3ForMemo { expression, index })
}

pub(crate) fn lower_vue3_if_directive_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    directive: &Vue3Directive,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    let condition =
        lower_vue3_optional_condition(directive, ast_node, &mut state.js, state.source_type);
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::If(HirIf {
            branches: Vec::new(),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3DomMirKind::If { condition },
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let body = if let Some(for_dir) = directive_by_name(element, "for") {
        lower_vue3_for_directive_to_dom_mir(
            ast_id,
            element,
            ast,
            ast_node,
            for_dir,
            Some(0),
            hir_id,
            mir_id,
            state,
        )
    } else if vue3_dom_is_structural_template(element) {
        lower_vue3_structural_template_body_to_dom_mir(
            ast_node,
            ast,
            hir_id,
            mir_id,
            Some(Vue3DomKey::Branch(0)),
            Vue3StructuralTemplateBodyKind::If,
            state,
        )
    } else {
        lower_vue3_non_control_element_to_dom_mir(
            ast_id, element, ast, ast_node, hir_id, mir_id, state,
        )
    };
    if let Some((body_hir, body_mir)) = body {
        inject_vue3_dom_key(body_mir, Vue3DomKey::Branch(0), state);
        if let Some(node) = state.hir.node_mut(hir_id) {
            if let HirNodeKind::If(hir_if) = &mut node.kind {
                hir_if.branches.push(HirIfBranch {
                    condition,
                    body: body_hir,
                });
            }
        }
    }
    Some((hir_id, mir_id))
}
