fn add_handler(
    events: &mut BTreeMap<String, Vec<Vue2EventHandler>>,
    mut name: String,
    value: String,
    mut modifiers: BTreeMap<String, bool>,
    modifier_order: Vec<String>,
    has_modifier_object: bool,
    dynamic: bool,
    prepend: bool,
    span: Option<Span>,
) {
    if modifiers.get("right").copied().unwrap_or(false) {
        if dynamic {
            name = format!("({name})==='click'?'contextmenu':({name})");
        } else if name == "click" {
            modifiers.remove("right");
            name = "contextmenu".into();
        }
    } else if modifiers.get("middle").copied().unwrap_or(false) {
        if dynamic {
            name = format!("({name})==='click'?'mouseup':({name})");
        } else if name == "click" {
            name = "mouseup".into();
        }
    }
    if modifiers.remove("capture").is_some() {
        name = prepend_vue2_event_modifier_marker("!", &name, dynamic);
    }
    if modifiers.remove("once").is_some() {
        name = prepend_vue2_event_modifier_marker("~", &name, dynamic);
    }
    if modifiers.remove("passive").is_some() {
        name = prepend_vue2_event_modifier_marker("&", &name, dynamic);
    }
    let handler = Vue2EventHandler {
        value: value.trim().to_string(),
        modifiers,
        modifier_order,
        has_modifier_object,
        dynamic,
        span,
    };
    let handlers = events.entry(name).or_default();
    if prepend {
        handlers.insert(0, handler);
    } else {
        handlers.push(handler);
    }
}

fn prepend_vue2_event_modifier_marker(symbol: &str, name: &str, dynamic: bool) -> String {
    if dynamic {
        format!("_p({name},{})", js_string(symbol))
    } else {
        format!("{symbol}{name}")
    }
}

fn gen_component_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    let mut value_expression = "$$v".to_string();
    if modifiers.get("trim").copied().unwrap_or(false) {
        value_expression = "(typeof $$v === 'string'? $$v.trim(): $$v)".into();
    }
    if modifiers.get("number").copied().unwrap_or(false) {
        value_expression = format!("_n({value_expression})");
    }
    let assignment = gen_assignment_code(value, &value_expression);
    element.model = Some(Vue2ComponentModel {
        value: format!("({value})"),
        expression: js_string(value),
        callback: format!("function ($$v) {{{assignment}}}"),
    });
}

fn add_dom_model_directive(
    element: &mut Vue2Element,
    raw_name: &str,
    value: &str,
    modifiers: &BTreeMap<String, bool>,
) {
    element.directives.push(Vue2Directive {
        name: "model".into(),
        raw_name: raw_name.into(),
        value: Some(value.into()),
        arg: None,
        is_dynamic_arg: false,
        modifiers: modifiers.clone(),
        span: element.span,
    });
}

fn gen_dom_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    let input_type = element.attrs_map.get("type").map(String::as_str);
    if element.tag == "select" {
        gen_select_model(element, value, modifiers);
    } else if element.tag == "input" && input_type == Some("checkbox") {
        gen_checkbox_model(element, value, modifiers);
    } else if element.tag == "input" && input_type == Some("radio") {
        gen_radio_model(element, value, modifiers);
    } else if matches!(element.tag.as_str(), "input" | "textarea") {
        gen_default_model(element, value, modifiers);
    }
}

fn add_dom_prop(element: &mut Vue2Element, name: &str, value: String) {
    element.props.push(Vue2Attribute {
        name: name.into(),
        value,
        span: element.span,
        dynamic: false,
    });
}

fn gen_checkbox_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    let value_binding = get_binding_attr(element, "value", true).unwrap_or_else(|| "null".into());
    let true_value_binding =
        get_binding_attr(element, "true-value", true).unwrap_or_else(|| "true".into());
    let false_value_binding =
        get_binding_attr(element, "false-value", true).unwrap_or_else(|| "false".into());
    let checked = format!(
        "Array.isArray({value})?_i({value},{value_binding})>-1{}",
        if true_value_binding == "true" {
            format!(":({value})")
        } else {
            format!(":_q({value},{true_value_binding})")
        }
    );
    add_dom_prop(element, "checked", checked);

    let array_value_binding = if modifiers.get("number").copied().unwrap_or(false) {
        format!("_n({value_binding})")
    } else {
        value_binding.clone()
    };
    let add_assignment = gen_assignment_code(value, "$$a.concat([$$v])");
    let remove_assignment = gen_assignment_code(value, "$$a.slice(0,$$i).concat($$a.slice($$i+1))");
    let fallback_assignment = gen_assignment_code(value, "$$c");
    let handler = format!(
        "var $$a={value},$$el=$event.target,$$c=$$el.checked?({true_value_binding}):({false_value_binding});\
if(Array.isArray($$a)){{var $$v={array_value_binding},$$i=_i($$a,$$v);\
if($$el.checked){{$$i<0&&({add_assignment})}}else{{$$i>-1&&({remove_assignment})}}}}\
else{{{fallback_assignment}}}"
    );
    add_handler(
        &mut element.events,
        "change".into(),
        handler,
        BTreeMap::new(),
        Vec::new(),
        false,
        false,
        true,
        None,
    );
}

fn gen_radio_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    let mut value_binding =
        get_binding_attr(element, "value", true).unwrap_or_else(|| "null".into());
    if modifiers.get("number").copied().unwrap_or(false) {
        value_binding = format!("_n({value_binding})");
    }
    add_dom_prop(element, "checked", format!("_q({value},{value_binding})"));
    add_handler(
        &mut element.events,
        "change".into(),
        gen_assignment_code(value, &value_binding),
        BTreeMap::new(),
        Vec::new(),
        false,
        false,
        true,
        None,
    );
}

fn gen_select_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    let selected_return = if modifiers.get("number").copied().unwrap_or(false) {
        "_n(val)"
    } else {
        "val"
    };
    let selected_val = format!(
        "Array.prototype.filter.call($event.target.options,function(o){{return o.selected}})\
.map(function(o){{var val = \"_value\" in o ? o._value : o.value;return {selected_return}}})"
    );
    let assignment = gen_assignment_code(
        value,
        "$event.target.multiple ? $$selectedVal : $$selectedVal[0]",
    );
    let handler = format!("var $$selectedVal = {selected_val}; {assignment}");
    add_handler(
        &mut element.events,
        "change".into(),
        handler,
        BTreeMap::new(),
        Vec::new(),
        false,
        false,
        true,
        None,
    );
}

fn gen_default_model(element: &mut Vue2Element, value: &str, modifiers: &BTreeMap<String, bool>) {
    add_dom_prop(element, "value", format!("({value})"));
    let assignment_value = if modifiers.get("trim").copied().unwrap_or(false) {
        "$event.target.value.trim()"
    } else {
        "$event.target.value"
    };
    let assignment_value = if modifiers.get("number").copied().unwrap_or(false) {
        format!("_n({assignment_value})")
    } else {
        assignment_value.into()
    };
    let assignment = gen_assignment_code(value, &assignment_value);
    let event = if modifiers.get("lazy").copied().unwrap_or(false) {
        "change"
    } else if element.attrs_map.get("type").map(String::as_str) == Some("range") {
        "__r"
    } else {
        "input"
    };
    let mut handler = String::new();
    if !modifiers.get("lazy").copied().unwrap_or(false)
        && element.attrs_map.get("type").map(String::as_str) != Some("range")
    {
        handler.push_str("if($event.target.composing)return;");
    }
    handler.push_str(&assignment);
    add_handler(
        &mut element.events,
        event.into(),
        handler,
        BTreeMap::new(),
        Vec::new(),
        false,
        false,
        true,
        None,
    );
    if modifiers.get("trim").copied().unwrap_or(false)
        || modifiers.get("number").copied().unwrap_or(false)
    {
        add_handler(
            &mut element.events,
            "blur".into(),
            "$forceUpdate()".into(),
            BTreeMap::new(),
            Vec::new(),
            false,
            false,
            false,
            None,
        );
    }
}

fn gen_assignment_code(value: &str, assignment: &str) -> String {
    let parsed_value = value.trim();
    if let Some(dot) = parsed_value.rfind('.') {
        if !parsed_value[dot + 1..].contains(']') && !parsed_value[dot + 1..].contains('[') {
            return format!(
                "$set({}, \"{}\", {assignment})",
                &parsed_value[..dot],
                &parsed_value[dot + 1..]
            );
        }
    }
    if parsed_value.ends_with(']') {
        if let Some(open) = find_model_bracket(parsed_value) {
            return format!(
                "$set({}, {}, {assignment})",
                &parsed_value[..open],
                &parsed_value[open + 1..parsed_value.len() - 1]
            );
        }
    }
    format!("{value}={assignment}")
}

fn find_model_bracket(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in value.char_indices().rev() {
        match ch {
            ']' => depth += 1,
            '[' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(source: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
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
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}
