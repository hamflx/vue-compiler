fn generate_render_mir(
    mir: &Vue2Mir,
    js: &JsAstStore,
    options: &Vue2CompileOptions,
    static_render_fns: &mut Vec<String>,
) -> String {
    let mut state = Vue2MirCodegenState {
        mir,
        js,
        options,
        static_render_fns,
        pre: false,
        parent_pre: false,
    };
    let code = mir
        .root_node()
        .and_then(|root| root.children.first().copied())
        .map(|root| {
            if mir_node_tag(mir, root).as_deref() == Some("script") {
                "null".into()
            } else {
                gen_mir_node(root, &mut state)
            }
        })
        .unwrap_or_else(|| "_c(\"div\")".into());
    format!("with(this){{return {code}}}")
}

struct Vue2MirCodegenState<'a> {
    mir: &'a Vue2Mir,
    js: &'a JsAstStore,
    options: &'a Vue2CompileOptions,
    static_render_fns: &'a mut Vec<String>,
    pre: bool,
    parent_pre: bool,
}

fn gen_mir_node(id: NodeId, state: &mut Vue2MirCodegenState<'_>) -> String {
    let Some(node) = state.mir.node(id) else {
        return "_e()".into();
    };
    match &node.kind {
        Vue2MirKind::Root(_) => node
            .children
            .first()
            .map(|child| gen_mir_node(*child, state))
            .unwrap_or_else(|| "_c(\"div\")".into()),
        Vue2MirKind::CreateElement(create) => gen_mir_create_element(id, create, state),
        Vue2MirKind::Text(text) => gen_mir_text(id, text, state),
        Vue2MirKind::Comment { value } => format!("_e({})", js_string(value)),
        Vue2MirKind::If(if_node) => gen_mir_if_conditions(&if_node.branches, state),
        Vue2MirKind::For(for_node) => gen_mir_for(for_node, state),
        Vue2MirKind::RenderStatic(render_static) => gen_mir_static(render_static, state),
        Vue2MirKind::Once(once) => {
            let code = gen_mir_node(once.body, state);
            let key = once
                .key
                .as_ref()
                .map(|key| render_mir_expr(key, state))
                .unwrap_or_else(|| "null".into());
            format!("_o({code},{},{key})", once.once_id)
        }
        Vue2MirKind::SlotOutlet(slot) => gen_mir_slot_outlet(id, slot, state),
        Vue2MirKind::ScopedSlot(slot) => gen_mir_scoped_slot(slot, state),
        Vue2MirKind::FilterCall { .. } | Vue2MirKind::Directive(_) => "_e()".into(),
    }
}

fn gen_mir_create_element(
    id: NodeId,
    create: &Vue2CreateElement,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    if create.is_template && !state.pre {
        return gen_mir_children(id, state, false).unwrap_or_else(|| "void 0".into());
    }

    let effective_pre = state.parent_pre || create.data.as_ref().is_some_and(|data| data.pre);
    let maybe_component = mir_create_is_component(create, state.options);
    let data = create
        .data
        .as_ref()
        .map(|data| gen_mir_data(data, mir_create_tag_literal(create), state, effective_pre))
        .or_else(|| {
            (effective_pre && maybe_component).then(|| {
                gen_mir_data(
                    &Vue2DataObject::default(),
                    mir_create_tag_literal(create),
                    state,
                    true,
                )
            })
        });
    let children = if create
        .data
        .as_ref()
        .and_then(|data| data.inline_template.as_ref())
        .is_some()
    {
        None
    } else {
        let original_parent_pre = state.parent_pre;
        state.parent_pre = effective_pre;
        let children = gen_mir_children(id, state, true);
        state.parent_pre = original_parent_pre;
        children
    };
    let tag = gen_mir_create_tag(create, state);
    let code = match (data, children) {
        (Some(data), Some(children)) => format!("_c({tag},{data},{children})"),
        (Some(data), None) => format!("_c({tag},{data})"),
        (None, Some(children)) => format!("_c({tag},{children})"),
        (None, None) => format!("_c({tag})"),
    };
    create
        .validation
        .as_ref()
        .map(|validation| wrap_validation_mir(validation, &code))
        .unwrap_or(code)
}

fn gen_mir_create_tag(create: &Vue2CreateElement, state: &Vue2MirCodegenState<'_>) -> String {
    match &create.tag {
        MirExpr::String(tag) => {
            let maybe_component =
                create.is_component || !is_reserved_tag_with_options(tag, state.options);
            if maybe_component {
                if let Some(binding) = binding_component_tag_name(tag, state.options) {
                    return binding;
                }
            }
            js_string_single(tag)
        }
        _ => render_mir_expr(&create.tag, state),
    }
}

fn mir_create_tag_literal(create: &Vue2CreateElement) -> Option<&str> {
    create
        .data
        .as_ref()
        .and_then(|data| data.tag.as_deref())
        .or(match &create.tag {
            MirExpr::String(tag) => Some(tag.as_str()),
            _ => None,
        })
}

fn mir_create_is_component(create: &Vue2CreateElement, options: &Vue2CompileOptions) -> bool {
    create.is_component
        || match &create.tag {
            MirExpr::String(tag) => !is_reserved_tag_with_options(tag, options),
            _ => true,
        }
}

fn gen_mir_text(id: NodeId, text: &Vue2TextCall, state: &Vue2MirCodegenState<'_>) -> String {
    let mut expression = render_mir_expr(&text.value, state);
    let mut filtered = false;
    if let Some(node) = state.mir.node(id) {
        for child in &node.children {
            if let Some(Vue2MirKind::FilterCall { name, args }) =
                state.mir.node(*child).map(|node| &node.kind)
            {
                let args = args
                    .iter()
                    .map(|arg| render_js_expr(state.js, *arg))
                    .collect::<Vec<_>>();
                expression = if args.is_empty() {
                    format!("_f({})({expression})", js_string(name))
                } else {
                    format!("_f({})({expression},{})", js_string(name), args.join(","))
                };
                filtered = true;
            }
        }
    }
    if filtered {
        format!("_v(_s({expression}))")
    } else {
        format!("_v({expression})")
    }
}

fn gen_mir_static(render_static: &Vue2RenderStatic, state: &mut Vue2MirCodegenState<'_>) -> String {
    let Some(body) = render_static.body else {
        return format!("_m({})", render_static.index);
    };
    let original_pre = state.pre;
    if state.parent_pre || mir_node_pre(state.mir, body) {
        state.pre = true;
    }
    let code = gen_mir_node(body, state);
    state.pre = original_pre;
    let index = render_static.index as usize;
    while state.static_render_fns.len() <= index {
        state.static_render_fns.push(String::new());
    }
    state.static_render_fns[index] = format!("with(this){{return {code}}}");
    if render_static.in_for {
        format!("_m({},true)", render_static.index)
    } else {
        format!("_m({})", render_static.index)
    }
}

fn gen_mir_if_conditions(
    conditions: &[Vue2IfMirBranch],
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let Some((condition, rest)) = conditions.split_first() else {
        return "_e()".into();
    };
    if let Some(exp) = condition.condition {
        format!(
            "({})?{}:{}",
            render_js_expr(state.js, exp),
            gen_mir_node(condition.body, state),
            gen_mir_if_conditions(rest, state)
        )
    } else {
        gen_mir_node(condition.body, state)
    }
}

fn gen_mir_for(for_node: &Vue2ForMir, state: &mut Vue2MirCodegenState<'_>) -> String {
    let source = render_js_expr(state.js, for_node.source);
    let alias = render_js_pattern(state.js, for_node.alias);
    let iterator1 = for_node
        .iterator1
        .map(|value| format!(",{}", render_js_pattern(state.js, value)))
        .unwrap_or_default();
    let iterator2 = for_node
        .iterator2
        .map(|value| format!(",{}", render_js_pattern(state.js, value)))
        .unwrap_or_default();
    let body = gen_mir_node(for_node.body, state);
    format!("_l(({source}),function({alias}{iterator1}{iterator2}){{return {body}}})")
}

fn gen_mir_data(
    data: &Vue2DataObject,
    tag_literal: Option<&str>,
    state: &mut Vue2MirCodegenState<'_>,
    effective_pre: bool,
) -> String {
    let mut parts = Vec::new();
    if let Some(dirs) = gen_mir_directives(&data.directives, state) {
        parts.push(dirs);
    }
    if let Some(key) = &data.key {
        parts.push(format!("key:{}", render_mir_expr(key, state)));
    }
    if let Some(ref_name) = &data.ref_name {
        parts.push(format!("ref:{}", render_mir_expr(ref_name, state)));
    }
    if data.ref_in_for {
        parts.push("refInFor:true".into());
    }
    if effective_pre {
        parts.push("pre:true".into());
    }
    if let Some(tag) = &data.tag {
        parts.push(format!("tag:{}", js_string(tag)));
    }
    if let Some(static_class) = &data.static_class {
        parts.push(format!(
            "staticClass:{}",
            render_mir_expr(static_class, state)
        ));
    }
    if let Some(class_binding) = &data.class_binding {
        parts.push(format!("class:{}", render_mir_expr(class_binding, state)));
    }
    if let Some(static_style) = &data.static_style {
        parts.push(format!(
            "staticStyle:{}",
            render_mir_expr(static_style, state)
        ));
    }
    if let Some(style_binding) = &data.style_binding {
        parts.push(format!("style:({})", render_mir_expr(style_binding, state)));
    }
    if !data.attrs.is_empty() {
        parts.push(format!(
            "attrs:{}",
            gen_mir_props(&data.attrs, PropValueKind::StaticAttribute, state)
        ));
    }
    if !data.dom_props.is_empty() {
        parts.push(format!(
            "domProps:{}",
            gen_mir_props(&data.dom_props, PropValueKind::Expression, state)
        ));
    }
    if !data.events.is_empty() {
        parts.push(gen_mir_handlers(&data.events, false, state));
    }
    if !data.native_events.is_empty() {
        parts.push(gen_mir_handlers(&data.native_events, true, state));
    }
    if let Some(slot) = &data.slot {
        parts.push(format!("slot:{}", render_mir_expr(slot, state)));
    }
    if !data.scoped_slots.is_empty() {
        parts.push(gen_mir_scoped_slots(&data.scoped_slots, state));
    }
    if let Some(model) = &data.model {
        parts.push(format!(
            "model:{{value:{},callback:{},expression:{}}}",
            render_mir_expr(&model.value, state),
            render_js_stmt(state.js, model.callback),
            model.expression
        ));
    }
    if let Some(inline) = &data.inline_template {
        if let Some(inline) = gen_mir_inline_template(inline, state) {
            parts.push(inline);
        }
    }
    if let Some(validate) = &data.validate {
        parts.push(format!(
            "validate:{{\"field\":{},\"groups\":{}}}",
            js_string(&validate.field),
            json_string_array(&validate.groups)
        ));
    }
    if !data.validators.is_empty() {
        parts.push(format!(
            "validators:{}",
            ast_validators_json(&data.validators)
        ));
    }

    let mut rendered = format!("{{{}}}", parts.join(","));
    let tag = tag_literal.unwrap_or("");
    if !data.dynamic_attrs.is_empty() {
        rendered = format!(
            "_b({rendered},{},{} )",
            js_string(tag),
            gen_mir_props(&data.dynamic_attrs, PropValueKind::Expression, state)
        )
        .replace("} )", "})");
    }
    if let Some(wrap) = &data.wrap_data {
        rendered = format!(
            "_b({rendered},{},{},{prop}{sync})",
            js_string_single(tag),
            render_mir_expr(&wrap.value, state),
            prop = wrap.prop,
            sync = if wrap.sync { ",true" } else { "" }
        );
    }
    if let Some(listeners) = &data.wrap_listeners {
        rendered = format!("_g({rendered},{})", render_mir_expr(listeners, state));
    }
    rendered
}

fn gen_mir_directives(
    directives: &[Vue2DirectiveRuntime],
    state: &Vue2MirCodegenState<'_>,
) -> Option<String> {
    if directives.is_empty() {
        return None;
    }
    let rendered = directives
        .iter()
        .map(|directive| {
            let mut fields = vec![
                format!("name:{}", js_string(&directive.name)),
                format!("rawName:{}", js_string(&directive.raw_name)),
            ];
            if let Some(value) = &directive.value {
                let value = render_mir_expr(value, state);
                fields.push(format!("value:({value})"));
                fields.push(format!("expression:{}", js_string(&value)));
            }
            if let Some(arg) = &directive.arg {
                if directive.is_dynamic_arg {
                    fields.push(format!("arg:{}", render_mir_expr(arg, state)));
                } else {
                    fields.push(format!(
                        "arg:{}",
                        js_string(&render_mir_string_arg(arg, state))
                    ));
                }
            }
            if !directive.modifiers.is_empty() {
                fields.push(format!(
                    "modifiers:{}",
                    modifiers_json(&directive.modifiers, Some(&directive.raw_name))
                ));
            }
            format!("{{{}}}", fields.join(","))
        })
        .collect::<Vec<_>>();
    Some(format!("directives:[{}]", rendered.join(",")))
}

fn gen_mir_props(
    attrs: &[Vue2DataProp],
    value_kind: PropValueKind,
    state: &Vue2MirCodegenState<'_>,
) -> String {
    let static_props = attrs
        .iter()
        .filter(|attr| !attr.dynamic)
        .map(|attr| {
            let value = render_mir_expr(&attr.value, state);
            let value = match value_kind {
                PropValueKind::StaticAttribute => transform_vue2_js_special_newlines(&value),
                PropValueKind::Expression => value,
            };
            format!("{}:{value}", js_string(&attr.name))
        })
        .collect::<Vec<_>>()
        .join(",");
    let dynamic_props = attrs
        .iter()
        .filter(|attr| attr.dynamic)
        .flat_map(|attr| [attr.name.clone(), render_mir_expr(&attr.value, state)])
        .collect::<Vec<_>>();
    if dynamic_props.is_empty() {
        format!("{{{static_props}}}")
    } else {
        format!("_d({{{static_props}}},[{}])", dynamic_props.join(","))
    }
}

fn gen_mir_handlers(
    events: &BTreeMap<String, Vec<vuec_ast::Vue2EventHandler>>,
    native: bool,
    state: &Vue2MirCodegenState<'_>,
) -> String {
    let prefix = if native { "nativeOn" } else { "on" };
    let mut entries = events.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(name, handlers)| vue2_event_order_key(name, handlers));
    let mut static_handlers = Vec::new();
    let mut dynamic_handlers = Vec::new();
    for (name, handlers) in entries {
        let code = if handlers.is_empty() {
            "function(){}".into()
        } else if handlers.len() == 1 {
            gen_mir_handler(&handlers[0], state)
        } else {
            format!(
                "[{}]",
                handlers
                    .iter()
                    .map(|handler| gen_mir_handler(handler, state))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        if handlers.len() == 1 && handlers[0].dynamic {
            dynamic_handlers.push(name.clone());
            dynamic_handlers.push(code);
        } else {
            static_handlers.push(format!("{}:{code}", js_string(name)));
        }
    }
    let static_handlers = format!("{{{}}}", static_handlers.join(","));
    if dynamic_handlers.is_empty() {
        format!("{prefix}:{static_handlers}")
    } else {
        format!(
            "{prefix}:_d({static_handlers},[{}])",
            dynamic_handlers.join(",")
        )
    }
}

fn vue2_event_order_key<'a>(
    name: &'a str,
    handlers: &[vuec_ast::Vue2EventHandler],
) -> (usize, usize, usize, usize, &'a str) {
    let span = handlers
        .iter()
        .find_map(|handler| handler.span)
        .map(|span| (span.start.0, span.end.0))
        .unwrap_or((usize::MAX, usize::MAX));
    (
        span.0,
        span.1,
        vue2_generated_event_rank(name, span),
        vue2_same_attr_event_rank(name),
        name,
    )
}

fn vue2_generated_event_rank(name: &str, span: (usize, usize)) -> usize {
    if span != (usize::MAX, usize::MAX) {
        return 0;
    }
    match name {
        "input" | "change" | "__r" => 0,
        "blur" => 1,
        _ => 0,
    }
}

fn vue2_same_attr_event_rank(name: &str) -> usize {
    if name
        .strip_prefix("update:")
        .is_some_and(|event| event.contains('-'))
    {
        1
    } else {
        0
    }
}

fn gen_mir_handler(
    handler: &vuec_ast::Vue2EventHandler,
    state: &Vue2MirCodegenState<'_>,
) -> String {
    let value = render_js_stmt(state.js, handler.value);
    let is_method_path = is_simple_path(&value);
    let is_function_expression = is_function_expression(&value);
    let is_function_invocation = is_function_invocation(&value);
    let has_modifier_object = handler.has_modifier_object || !handler.modifiers.is_empty();
    if !has_modifier_object {
        if is_method_path || is_function_expression {
            return value;
        }
        if is_function_invocation {
            return format!("function($event){{return {value}}}");
        }
        return format!("function($event){{{value}}}");
    }

    let mut code = String::new();
    let mut modifier_code = String::new();
    let mut keys = Vec::new();
    let modifier_order = if handler.modifier_order.is_empty() {
        handler.modifiers.keys().cloned().collect::<Vec<_>>()
    } else {
        handler
            .modifier_order
            .iter()
            .filter(|key| handler.modifiers.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>()
    };
    for key in &modifier_order {
        match key.as_str() {
            "stop" => modifier_code.push_str("$event.stopPropagation();"),
            "prevent" => modifier_code.push_str("$event.preventDefault();"),
            "self" => {
                modifier_code.push_str("if($event.target !== $event.currentTarget)return null;")
            }
            "ctrl" => modifier_code.push_str("if(!$event.ctrlKey)return null;"),
            "shift" => modifier_code.push_str("if(!$event.shiftKey)return null;"),
            "alt" => modifier_code.push_str("if(!$event.altKey)return null;"),
            "meta" => modifier_code.push_str("if(!$event.metaKey)return null;"),
            "left" => {
                modifier_code.push_str("if('button' in $event && $event.button !== 0)return null;");
                keys.push(key.clone());
            }
            "middle" => {
                modifier_code.push_str("if('button' in $event && $event.button !== 1)return null;")
            }
            "right" => {
                modifier_code.push_str("if('button' in $event && $event.button !== 2)return null;");
                keys.push(key.clone());
            }
            "exact" => {
                let guards = ["ctrl", "shift", "alt", "meta"]
                    .into_iter()
                    .filter(|modifier| !handler.modifiers.contains_key(*modifier))
                    .map(|modifier| format!("$event.{modifier}Key"))
                    .collect::<Vec<_>>()
                    .join("||");
                if !guards.is_empty() {
                    modifier_code.push_str(&format!("if({guards})return null;"));
                }
            }
            _ => keys.push(key.clone()),
        }
    }
    if !keys.is_empty() {
        code.push_str(&gen_key_filter(&keys));
    }
    code.push_str(&modifier_code);
    let handler_code = if is_method_path {
        format!("return {value}.apply(null, arguments)")
    } else if is_function_expression {
        format!("return ({value}).apply(null, arguments)")
    } else if is_function_invocation {
        format!("return {value}")
    } else {
        value
    };
    format!("function($event){{{code}{handler_code}}}")
}

fn gen_mir_children(
    parent: NodeId,
    state: &mut Vue2MirCodegenState<'_>,
    check_skip: bool,
) -> Option<String> {
    let children = renderable_mir_children(parent, state.mir);
    gen_mir_children_from_ids(&children, state, check_skip)
}

fn gen_mir_children_from_ids(
    children: &[NodeId],
    state: &mut Vue2MirCodegenState<'_>,
    check_skip: bool,
) -> Option<String> {
    if children.is_empty() {
        return None;
    }
    if children.len() == 1 && mir_for_child_can_skip_array(state.mir, children[0]) {
        let normalization = if check_skip {
            if mir_node_is_component(
                state.mir,
                mir_for_body_node(state.mir, children[0]).unwrap_or(children[0]),
                state.options,
            ) {
                ",1"
            } else {
                ",0"
            }
        } else {
            ""
        };
        let generated = gen_mir_node(children[0], state);
        return Some(format!("{generated}{normalization}"));
    }
    let nodes = children
        .iter()
        .map(|child| gen_mir_node(*child, state))
        .collect::<Vec<_>>();
    let normalization = if check_skip {
        get_mir_normalization_type(children, state.mir, state.options)
    } else {
        0
    };
    if normalization > 0 {
        Some(format!("[{}],{}", nodes.join(","), normalization))
    } else {
        Some(format!("[{}]", nodes.join(",")))
    }
}

fn gen_mir_slot_outlet(
    id: NodeId,
    slot: &Vue2SlotOutlet,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let name = render_mir_expr(&slot.name, state);
    let children = gen_mir_children(id, state, false);
    let props = (!slot.props.is_empty())
        .then(|| gen_mir_props(&slot.props, PropValueKind::Expression, state));
    let mut code = format!("_t({name}");
    if let Some(children) = children {
        code.push_str(&format!(",function(){{return {children}}}"));
    } else if props.is_some() || slot.bind.is_some() {
        code.push_str(",null");
    }
    if let Some(props) = props {
        code.push_str(&format!(",{props}"));
    }
    if let Some(bind) = &slot.bind {
        if slot.props.is_empty() {
            code.push_str(",null");
        }
        code.push_str(&format!(",{}", render_mir_expr(bind, state)));
    }
    code.push(')');
    code
}

fn gen_mir_inline_template(
    inline: &Vue2InlineTemplate,
    state: &Vue2MirCodegenState<'_>,
) -> Option<String> {
    let body = inline.body?;
    let mut static_render_fns = Vec::new();
    let mut nested = Vue2MirCodegenState {
        mir: state.mir,
        js: state.js,
        options: state.options,
        static_render_fns: &mut static_render_fns,
        pre: false,
        parent_pre: false,
    };
    let code = gen_mir_node(body, &mut nested);
    let render = format!("with(this){{return {code}}}");
    let static_render_fns = static_render_fns
        .into_iter()
        .map(|code| format!("function(){{{code}}}"))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "inlineTemplate:{{render:function(){{{render}}},staticRenderFns:[{static_render_fns}]}}"
    ))
}

fn gen_mir_scoped_slots(slots: &[Vue2ScopedSlot], state: &mut Vue2MirCodegenState<'_>) -> String {
    let rendered = slots
        .iter()
        .map(|slot| gen_mir_scoped_slot(slot, state))
        .collect::<Vec<_>>()
        .join(",");
    if slots.iter().any(|slot| slot.force_update) {
        format!("scopedSlots:_u([{rendered}],null,true)")
    } else if slots.iter().any(|slot| slot.needs_key) {
        format!(
            "scopedSlots:_u([{rendered}],null,false,{})",
            vue2_hash_scoped_slots(&rendered)
        )
    } else {
        format!("scopedSlots:_u([{rendered}])")
    }
}

fn vue2_hash_scoped_slots(value: &str) -> u32 {
    let mut hash = 5381u32;
    let units = value.encode_utf16().collect::<Vec<_>>();
    for unit in units.iter().rev() {
        hash = hash.wrapping_mul(33) ^ *unit as u32;
    }
    hash
}

fn gen_mir_scoped_slot(slot: &Vue2ScopedSlot, state: &mut Vue2MirCodegenState<'_>) -> String {
    if let Some(condition) = slot.condition {
        let alternate = gen_mir_scoped_slot_branches(&slot.branches, state);
        return format!(
            "({})?{}:{alternate}",
            render_js_expr(state.js, condition),
            gen_mir_scoped_slot_object(slot, state)
        );
    }
    if let Some(source) = slot.for_source {
        let alias = slot
            .for_alias
            .map(|alias| render_js_pattern(state.js, alias))
            .unwrap_or_else(|| "item".into());
        let iterator1 = slot
            .for_iterator1
            .map(|value| format!(",{}", render_js_pattern(state.js, value)))
            .unwrap_or_default();
        let iterator2 = slot
            .for_iterator2
            .map(|value| format!(",{}", render_js_pattern(state.js, value)))
            .unwrap_or_default();
        let body = gen_mir_scoped_slot_object(slot, state);
        return format!(
            "_l(({}),function({alias}{iterator1}{iterator2}){{return {body}}})",
            render_js_expr(state.js, source)
        );
    }
    gen_mir_scoped_slot_object(slot, state)
}

fn gen_mir_scoped_slot_branches(
    branches: &[Vue2ScopedSlotBranch],
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let Some((first, rest)) = branches.split_first() else {
        return "null".into();
    };
    if let Some(condition) = first.condition {
        let alternate = gen_mir_scoped_slot_branches(rest, state);
        format!(
            "({})?{}:{alternate}",
            render_js_expr(state.js, condition),
            gen_mir_scoped_slot_object(&first.slot, state)
        )
    } else {
        gen_mir_scoped_slot_object(&first.slot, state)
    }
}

fn gen_mir_scoped_slot_object(
    slot: &Vue2ScopedSlot,
    state: &mut Vue2MirCodegenState<'_>,
) -> String {
    let scope = match slot.params {
        Some(params) => {
            let scope = render_js_pattern(state.js, params);
            if scope == "_empty_" {
                String::new()
            } else {
                scope
            }
        }
        None if slot.proxy => String::new(),
        None => "undefined".into(),
    };
    let body = gen_mir_scoped_slot_body(slot, state);
    let proxy = if slot.proxy { ",proxy:true" } else { "" };
    format!(
        "{{key:{},fn:function({scope}){{return {body}}}{proxy}}}",
        render_mir_expr(&slot.name, state)
    )
}

fn gen_mir_scoped_slot_body(slot: &Vue2ScopedSlot, state: &mut Vue2MirCodegenState<'_>) -> String {
    if slot.body_is_fragment {
        let children = gen_mir_children_from_ids(&slot.body, state, false)
            .unwrap_or_else(|| "undefined".into());
        if let Some(condition) = slot.legacy_condition {
            format!(
                "({})?{children}:undefined",
                render_js_expr(state.js, condition)
            )
        } else {
            children
        }
    } else {
        slot.body
            .first()
            .map(|body| gen_mir_node(*body, state))
            .unwrap_or_else(|| "undefined".into())
    }
}

fn renderable_mir_children(parent: NodeId, mir: &Vue2Mir) -> Vec<NodeId> {
    mir.node(parent)
        .map(|node| {
            node.children
                .iter()
                .copied()
                .filter(|child| {
                    !matches!(
                        mir.node(*child).map(|node| &node.kind),
                        Some(Vue2MirKind::ScopedSlot(_))
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mir_for_child_can_skip_array(mir: &Vue2Mir, child: NodeId) -> bool {
    match mir.node(child).map(|node| &node.kind) {
        Some(Vue2MirKind::For(for_node)) => {
            mir_for_body_node(mir, for_node.body).is_some_and(|body| {
                !mir_node_is_template(mir, body) && !mir_node_is_slot_outlet(mir, body)
            })
        }
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_for_child_can_skip_array(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_for_child_can_skip_array(mir, once.body),
        _ => false,
    }
}

fn mir_for_body_node(mir: &Vue2Mir, child: NodeId) -> Option<NodeId> {
    match mir.node(child).map(|node| &node.kind) {
        Some(Vue2MirKind::For(for_node)) => Some(for_node.body),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static.body,
        Some(Vue2MirKind::Once(once)) => Some(once.body),
        _ => Some(child),
    }
}

fn get_mir_normalization_type(
    children: &[NodeId],
    mir: &Vue2Mir,
    options: &Vue2CompileOptions,
) -> u8 {
    let mut result = 0;
    for child in children {
        if mir_node_contains_for(mir, *child)
            || mir_node_is_template(mir, *child)
            || mir_node_is_slot_outlet(mir, *child)
        {
            return 2;
        }
        if mir_node_is_component(mir, *child, options) {
            result = 1;
        }
    }
    result
}

fn mir_node_contains_for(mir: &Vue2Mir, id: NodeId) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::For(_)) => true,
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_contains_for(mir, branch.body)),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_contains_for(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_node_contains_for(mir, once.body),
        _ => false,
    }
}

fn mir_node_tag(mir: &Vue2Mir, id: NodeId) -> Option<String> {
    match mir.node(id).map(|node| &node.kind)? {
        Vue2MirKind::CreateElement(create) => match &create.tag {
            MirExpr::String(tag) => Some(tag.clone()),
            _ => None,
        },
        Vue2MirKind::RenderStatic(render_static) => {
            render_static.body.and_then(|body| mir_node_tag(mir, body))
        }
        Vue2MirKind::Once(once) => mir_node_tag(mir, once.body),
        Vue2MirKind::For(for_node) => mir_node_tag(mir, for_node.body),
        _ => None,
    }
}

fn mir_node_pre(mir: &Vue2Mir, id: NodeId) -> bool {
    mir.node(id)
        .and_then(|node| match &node.kind {
            Vue2MirKind::CreateElement(create) => create.data.as_ref(),
            _ => None,
        })
        .is_some_and(|data| data.pre)
}

fn mir_node_is_template(mir: &Vue2Mir, id: NodeId) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::CreateElement(create)) => {
            create.is_template || matches!(&create.tag, MirExpr::String(tag) if tag == "template")
        }
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_is_template(mir, branch.body)),
        Some(Vue2MirKind::For(for_node)) => mir_node_is_template(mir, for_node.body),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_is_template(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_node_is_template(mir, once.body),
        _ => false,
    }
}

fn mir_node_is_slot_outlet(mir: &Vue2Mir, id: NodeId) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::SlotOutlet(_)) => true,
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_is_slot_outlet(mir, branch.body)),
        Some(Vue2MirKind::For(for_node)) => mir_node_is_slot_outlet(mir, for_node.body),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_is_slot_outlet(mir, body)),
        Some(Vue2MirKind::Once(once)) => mir_node_is_slot_outlet(mir, once.body),
        _ => false,
    }
}

fn mir_node_is_component(mir: &Vue2Mir, id: NodeId, options: &Vue2CompileOptions) -> bool {
    match mir.node(id).map(|node| &node.kind) {
        Some(Vue2MirKind::CreateElement(create)) => {
            create.is_component
                || matches!(
                    &create.tag,
                    MirExpr::String(tag) if !is_reserved_tag_with_options(tag, options)
                )
        }
        Some(Vue2MirKind::If(if_node)) => if_node
            .branches
            .iter()
            .any(|branch| mir_node_is_component(mir, branch.body, options)),
        Some(Vue2MirKind::For(for_node)) => mir_node_is_component(mir, for_node.body, options),
        Some(Vue2MirKind::RenderStatic(render_static)) => render_static
            .body
            .is_some_and(|body| mir_node_is_component(mir, body, options)),
        Some(Vue2MirKind::Once(once)) => mir_node_is_component(mir, once.body, options),
        _ => false,
    }
}

fn render_mir_expr(expr: &MirExpr, state: &Vue2MirCodegenState<'_>) -> String {
    match expr {
        MirExpr::String(value) => js_string(value),
        MirExpr::Bool(value) => value.to_string(),
        MirExpr::Null => "null".into(),
        MirExpr::JsExpr(id) => render_js_expr(state.js, *id),
        MirExpr::Helper(helper) => format!("{helper:?}"),
    }
}

fn render_mir_string_arg(expr: &MirExpr, state: &Vue2MirCodegenState<'_>) -> String {
    match expr {
        MirExpr::String(value) => value.clone(),
        _ => render_mir_expr(expr, state),
    }
}

fn render_js_expr(js: &JsAstStore, id: JsExprId) -> String {
    js.expr_entry(id)
        .map(|entry| entry.source.as_str().to_string())
        .unwrap_or_default()
}

fn render_js_stmt(js: &JsAstStore, id: JsStmtId) -> String {
    js.stmt_entry(id)
        .map(|entry| entry.source.as_str().to_string())
        .unwrap_or_default()
}

fn render_js_pattern(js: &JsAstStore, id: JsPatternId) -> String {
    js.pattern_entry(id)
        .map(|entry| entry.source.as_str().to_string())
        .unwrap_or_default()
}

fn binding_component_tag_name(tag: &str, options: &Vue2CompileOptions) -> Option<String> {
    if !options.bindings_is_script_setup || options.bindings.is_empty() {
        return None;
    }
    check_binding_type(&options.bindings, tag)
}

fn wrap_validation_mir(validation: &Vue2ValidationData, child_code: &str) -> String {
    let field = validation
        .validate
        .as_ref()
        .map(|validate| validate.field.clone())
        .unwrap_or_default();
    let groups = validation
        .validate
        .as_ref()
        .map(|validate| validate.groups.clone())
        .unwrap_or_default();
    format!(
        "_c('validate',{{props:{{field:{},groups:{},validators:{},result:{},child:{child_code}}}}})",
        js_string(&field),
        json_string_array(&groups),
        ast_validators_json(&validation.validators),
        ast_validation_result_json(&validation.validators)
    )
}

#[derive(Clone, Copy)]
enum PropValueKind {
    StaticAttribute,
    Expression,
}

fn gen_key_filter(keys: &[String]) -> String {
    format!(
        "if(!$event.type.indexOf('key')&&{})return null;",
        keys.iter()
            .map(|key| {
                key.parse::<u32>().map_or_else(
                    |_| match key.as_str() {
                        "tab" => "_k($event.keyCode,\"tab\",9,$event.key,\"Tab\")".into(),
                        "enter" => "_k($event.keyCode,\"enter\",13,$event.key,\"Enter\")".into(),
                        "delete" => "_k($event.keyCode,\"delete\",[8,46],$event.key,[\"Backspace\",\"Delete\",\"Del\"])".into(),
                        "esc" => "_k($event.keyCode,\"esc\",27,$event.key,[\"Esc\",\"Escape\"])".into(),
                        "space" => "_k($event.keyCode,\"space\",32,$event.key,[\" \",\"Spacebar\"])".into(),
                        "up" => "_k($event.keyCode,\"up\",38,$event.key,[\"Up\",\"ArrowUp\"])".into(),
                        "left" => "_k($event.keyCode,\"left\",37,$event.key,[\"Left\",\"ArrowLeft\"])".into(),
                        "right" => "_k($event.keyCode,\"right\",39,$event.key,[\"Right\",\"ArrowRight\"])".into(),
                        "down" => "_k($event.keyCode,\"down\",40,$event.key,[\"Down\",\"ArrowDown\"])".into(),
                        _ => format!("_k($event.keyCode,{},{},$event.key,{})", js_string(key), "undefined", "undefined"),
                    },
                    |code| format!("$event.keyCode!=={code}"),
                )
            })
            .collect::<Vec<_>>()
            .join("&&")
    )
}

fn validate_expressions(
    root: Option<&Vue2Element>,
    js: &JsAstStore,
    diagnostics: &mut DiagnosticSink,
) {
    let Some(root) = root else {
        return;
    };
    validate_element_expressions(root, js, diagnostics);
}
