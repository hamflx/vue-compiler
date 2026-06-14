struct DomContentDirectiveProjection {
    key: &'static str,
    key_loc: Option<&'static str>,
    missing_expression_code: u8,
    with_children_code: u8,
    wrap_dynamic_text: bool,
}

fn transform_dom_content_directive_projection(
    payload: &Value,
    projection: DomContentDirectiveProjection,
) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let exp = dir.get("exp").filter(|exp| !exp.is_null());
    let has_children = payload
        .get("node")
        .and_then(|node| node.get("children"))
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty());

    let mut errors = Vec::new();
    if exp.is_none() {
        errors.push(json!({
            "code": projection.missing_expression_code,
            "loc": "dir",
        }));
    }
    if has_children {
        errors.push(json!({
            "code": projection.with_children_code,
            "loc": "dir",
        }));
    }

    let value = match exp {
        Some(exp) if projection.wrap_dynamic_text && !dom_directive_exp_is_constant(exp) => {
            json!({
                "kind": "displayString",
                "argument": {
                    "kind": "node",
                    "path": "dir.exp",
                },
                "loc": "dir",
            })
        }
        Some(_) => json!({
            "kind": "node",
            "path": "dir.exp",
        }),
        None => json!({
            "kind": "simple",
            "content": "",
            "isStatic": true,
        }),
    };

    let mut prop = json!({
        "key": projection.key,
        "value": value,
    });
    if let Some(key_loc) = projection.key_loc {
        prop["keyLoc"] = json!(key_loc);
    }

    json!({
        "props": [prop],
        "errors": errors,
        "clearChildren": has_children,
    })
}

fn dom_directive_exp_is_constant(exp: &Value) -> bool {
    exp.get("constType")
        .and_then(Value::as_i64)
        .is_some_and(|constant_type| constant_type > 0)
}

#[derive(Default)]
struct DomEventModifiers {
    key_modifiers: Vec<String>,
    non_key_modifiers: Vec<String>,
    event_option_modifiers: Vec<String>,
}

fn dom_directive_modifiers(dir: &Value) -> Vec<String> {
    dir.get("modifiers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|modifier| {
            modifier
                .as_str()
                .or_else(|| modifier.get("content").and_then(Value::as_str))
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn dom_resolve_event_modifiers(key: &Value, raw_modifiers: &[String]) -> DomEventModifiers {
    let mut modifiers = DomEventModifiers::default();
    for modifier in raw_modifiers {
        if dom_event_option_modifier(modifier) {
            modifiers.event_option_modifiers.push(modifier.clone());
            continue;
        }

        if dom_maybe_key_modifier(modifier) {
            if dom_projection_is_static_expression(key) {
                if dom_projection_is_keyboard_event(key) {
                    modifiers.key_modifiers.push(modifier.clone());
                } else {
                    modifiers.non_key_modifiers.push(modifier.clone());
                }
            } else {
                modifiers.key_modifiers.push(modifier.clone());
                modifiers.non_key_modifiers.push(modifier.clone());
            }
            continue;
        }

        if dom_non_key_modifier(modifier) {
            modifiers.non_key_modifiers.push(modifier.clone());
        } else {
            modifiers.key_modifiers.push(modifier.clone());
        }
    }
    modifiers
}

fn dom_event_option_modifier(modifier: &str) -> bool {
    matches!(modifier, "passive" | "once" | "capture")
}

fn dom_non_key_modifier(modifier: &str) -> bool {
    matches!(
        modifier,
        "stop" | "prevent" | "self" | "ctrl" | "shift" | "alt" | "meta" | "exact" | "middle"
    )
}

fn dom_maybe_key_modifier(modifier: &str) -> bool {
    matches!(modifier, "left" | "right")
}

fn dom_projection_is_static_expression(projection: &Value) -> bool {
    match projection.get("kind").and_then(Value::as_str) {
        Some("static") => true,
        Some("simple") => projection
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

fn dom_projection_is_keyboard_event(projection: &Value) -> bool {
    dom_projection_static_content(projection)
        .map(|content| {
            matches!(
                content.to_ascii_lowercase().as_str(),
                "onkeyup" | "onkeydown" | "onkeypress"
            )
        })
        .unwrap_or(false)
}

fn dom_projection_static_content(projection: &Value) -> Option<&str> {
    if dom_projection_is_static_expression(projection) {
        projection.get("content").and_then(Value::as_str)
    } else {
        None
    }
}

fn dom_transform_click_projection(key: Value, event: &str) -> Value {
    if dom_projection_static_content(&key)
        .is_some_and(|content| content.eq_ignore_ascii_case("onClick"))
    {
        return json!({
            "kind": "simple",
            "content": event,
            "isStatic": true,
            "loc": key.get("loc").cloned().unwrap_or(Value::Null),
        });
    }

    if key.get("kind").and_then(Value::as_str) != Some("simple") {
        return json!({
            "kind": "compound",
            "children": [
                "(",
                key.clone(),
                format!(") === \"onClick\" ? \"{event}\" : ("),
                key,
                ")",
            ],
        });
    }
    key
}

fn dom_helper_call_projection(helper: &str, arguments: Vec<Value>) -> Value {
    json!({
        "kind": "call",
        "callee": helper,
        "arguments": arguments,
    })
}

fn dom_event_option_key_projection(key: Value, postfix: &str) -> Value {
    if dom_projection_is_static_expression(&key) {
        let content = dom_projection_static_content(&key)
            .unwrap_or("")
            .to_string();
        let mut next = key;
        next["kind"] = json!("simple");
        next["content"] = json!(format!("{content}{postfix}"));
        next["isStatic"] = json!(true);
        return next;
    }

    json!({
        "kind": "compound",
        "children": [
            "(",
            key,
            format!(") + \"{postfix}\""),
        ],
    })
}

fn dom_json_string_array(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn dom_capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn dom_normalize_core_model_projection(mut projection: Value, dir: &Value) -> Value {
    let errors = projection
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(|error| error.as_u64().map(|code| dom_core_model_error(code, dir)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    projection["errors"] = json!(errors);
    projection
}

fn dom_core_model_error(code: u64, dir: &Value) -> Value {
    let loc = if code == 41 {
        dir.get("loc").cloned().unwrap_or(Value::Null)
    } else {
        dir.get("exp")
            .and_then(|exp| exp.get("loc"))
            .cloned()
            .or_else(|| dir.get("loc").cloned())
            .unwrap_or(Value::Null)
    };
    json!({
        "code": code,
        "loc": loc,
    })
}

enum DomModelInputType<'a> {
    None,
    Dynamic,
    PresentWithoutValue,
    Static(&'a str),
}

fn dom_model_input_type(node: &Value) -> DomModelInputType<'_> {
    let Some(props) = node.get("props").and_then(Value::as_array) else {
        return DomModelInputType::None;
    };
    for prop in props {
        if json_u64(prop, "type") == Some(6) && json_str(prop, "name") == Some("type") {
            return prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .map(DomModelInputType::Static)
                .unwrap_or(DomModelInputType::PresentWithoutValue);
        }
        if json_u64(prop, "type") == Some(7)
            && json_str(prop, "name") == Some("bind")
            && prop.get("exp").is_some_and(|exp| !exp.is_null())
            && prop
                .get("arg")
                .filter(|arg| !arg.is_null())
                .is_some_and(|arg| {
                    json_bool(arg, "isStatic") && json_str(arg, "content") == Some("type")
                })
        {
            return DomModelInputType::Dynamic;
        }
    }
    if dom_model_has_dynamic_key_bind(props) {
        return DomModelInputType::Dynamic;
    }
    DomModelInputType::None
}

fn dom_model_has_dynamic_key_bind(props: &[Value]) -> bool {
    props.iter().any(|prop| {
        json_u64(prop, "type") == Some(7)
            && json_str(prop, "name") == Some("bind")
            && (prop.get("arg").is_none_or(Value::is_null)
                || prop
                    .get("arg")
                    .is_some_and(|arg| !json_bool(arg, "isStatic")))
    })
}

fn dom_model_dynamic_value_binding_loc(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_u64(prop, "type") == Some(7)
                && json_str(prop, "name") == Some("bind")
                && prop.get("exp").is_some_and(|exp| !exp.is_null())
                && prop
                    .get("arg")
                    .filter(|arg| !arg.is_null())
                    .is_some_and(|arg| {
                        json_bool(arg, "isStatic") && json_str(arg, "content") == Some("value")
                    })
        })
        .map(|prop| prop.get("loc").cloned().unwrap_or(Value::Null))
}

fn dom_filter_native_model_props(props: Vec<Value>) -> Vec<Value> {
    props
        .into_iter()
        .filter(|prop| {
            prop.get("key")
                .is_none_or(|key| json_str(key, "content") != Some("modelValue"))
        })
        .collect()
}

fn transition_json_visible_child_indices(children: &[Value]) -> Vec<usize> {
    children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| transition_json_child_is_visible(child).then_some(index))
        .collect()
}

fn transition_json_child_is_visible(child: &Value) -> bool {
    match json_u64(child, "type") {
        Some(3) => false,
        Some(2) => json_str(child, "content")
            .or_else(|| json_str(child, "value"))
            .is_none_or(|text| !text.chars().all(is_html_whitespace)),
        _ => true,
    }
}

fn transition_json_child_sequence_is_invalid(children: &[&Value], empty_is_invalid: bool) -> bool {
    if children.is_empty() {
        return empty_is_invalid;
    }
    children.len() != 1 || transition_json_child_is_invalid(children[0])
}

fn transition_json_child_is_invalid(child: &Value) -> bool {
    if json_u64(child, "type") == Some(11) {
        return true;
    }
    if json_u64(child, "type") == Some(9) {
        return child
            .get("branches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(transition_json_if_branch_is_invalid);
    }
    if json_u64(child, "type") == Some(1) {
        return child
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| transition_json_props_have_directive(props, "for"));
    }
    false
}

fn transition_json_if_branch_is_invalid(branch: &Value) -> bool {
    let visible_indices = branch
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            transition_json_visible_child_indices(children)
                .into_iter()
                .filter_map(|index| children.get(index))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    transition_json_child_sequence_is_invalid(&visible_indices, true)
}

fn transition_json_single_child_has_v_show(children: &[&Value]) -> bool {
    let [child] = children else {
        return false;
    };
    if json_u64(child, "type") != Some(1) {
        return false;
    }
    child
        .get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| transition_json_props_have_directive(props, "show"))
}

fn transition_json_props_have_directive(props: &[Value], name: &str) -> bool {
    props
        .iter()
        .any(|prop| json_u64(prop, "type") == Some(7) && json_str(prop, "name") == Some(name))
}

fn transition_json_error_loc(children: &[&Value]) -> Option<Value> {
    let first = children.first()?.get("loc")?;
    let last = children.last()?.get("loc")?;
    Some(json!({
        "start": first.get("start").cloned().unwrap_or(Value::Null),
        "end": last.get("end").cloned().unwrap_or(Value::Null),
        "source": "",
    }))
}

fn is_valid_html_nesting(parent: &str, child: &str) -> bool {
    if parent == "template" {
        return true;
    }
    if let Some(children) = html_nesting_only_valid_children(parent) {
        return children.contains(&child);
    }
    if let Some(parents) = html_nesting_only_valid_parents(child) {
        return parents.contains(&parent);
    }
    if let Some(children) = html_nesting_known_invalid_children(parent) {
        if children.contains(&child) {
            return false;
        }
    }
    if let Some(parents) = html_nesting_known_invalid_parents(child) {
        if parents.contains(&parent) {
            return false;
        }
    }
    true
}

fn html_nesting_only_valid_children(parent: &str) -> Option<&'static [&'static str]> {
    match parent {
        "head" => Some(&[
            "base",
            "basefront",
            "bgsound",
            "link",
            "meta",
            "title",
            "noscript",
            "noframes",
            "style",
            "script",
            "template",
        ]),
        "optgroup" => Some(&["option"]),
        "select" => Some(&["optgroup", "option", "hr"]),
        "table" => Some(&["caption", "colgroup", "tbody", "tfoot", "thead"]),
        "tr" => Some(&["td", "th"]),
        "colgroup" => Some(&["col"]),
        "tbody" | "thead" | "tfoot" => Some(&["tr"]),
        "script" | "iframe" | "option" | "textarea" | "style" | "title" => Some(&[]),
        _ => None,
    }
}

fn html_nesting_only_valid_parents(child: &str) -> Option<&'static [&'static str]> {
    match child {
        "html" => Some(&[]),
        "body" | "head" => Some(&["html"]),
        "td" | "th" => Some(&["tr"]),
        "colgroup" | "caption" | "tbody" | "tfoot" | "thead" => Some(&["table"]),
        "col" => Some(&["colgroup"]),
        "tr" => Some(&["tbody", "thead", "tfoot"]),
        "dd" | "dt" => Some(&["dl", "div"]),
        "figcaption" => Some(&["figure"]),
        "summary" => Some(&["details"]),
        "area" => Some(&["map"]),
        _ => None,
    }
}

fn html_nesting_known_invalid_children(parent: &str) -> Option<&'static [&'static str]> {
    match parent {
        "p" => Some(&[
            "address",
            "article",
            "aside",
            "blockquote",
            "center",
            "details",
            "dialog",
            "dir",
            "div",
            "dl",
            "fieldset",
            "figure",
            "footer",
            "form",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "header",
            "hgroup",
            "hr",
            "li",
            "main",
            "nav",
            "menu",
            "ol",
            "p",
            "pre",
            "section",
            "table",
            "ul",
        ]),
        "svg" => Some(&[
            "b",
            "blockquote",
            "br",
            "code",
            "dd",
            "div",
            "dl",
            "dt",
            "em",
            "embed",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "hr",
            "i",
            "img",
            "li",
            "menu",
            "meta",
            "ol",
            "p",
            "pre",
            "ruby",
            "s",
            "small",
            "span",
            "strong",
            "sub",
            "sup",
            "table",
            "u",
            "ul",
            "var",
        ]),
        _ => None,
    }
}

fn html_nesting_known_invalid_parents(child: &str) -> Option<&'static [&'static str]> {
    match child {
        "a" => Some(&["a"]),
        "button" => Some(&["button"]),
        "dd" | "dt" => Some(&["dd", "dt"]),
        "form" => Some(&["form"]),
        "li" => Some(&["li"]),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some(&["h1", "h2", "h3", "h4", "h5", "h6"]),
        _ => None,
    }
}
