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
