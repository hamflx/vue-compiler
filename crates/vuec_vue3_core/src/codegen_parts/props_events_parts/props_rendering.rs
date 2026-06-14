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
