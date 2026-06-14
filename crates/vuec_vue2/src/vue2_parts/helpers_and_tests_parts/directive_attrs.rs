fn single_default_interpolation(text: &str) -> Option<&str> {
    let inner = text.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    (!inner.contains("{{") && !inner.contains("}}")).then_some(inner)
}

fn get_binding_attr(element: &mut Vue2Element, name: &str, get_static: bool) -> Option<String> {
    get_binding_attr_with_span(element, name, get_static).map(|(value, _)| value)
}

fn get_binding_attr_with_span(
    element: &mut Vue2Element,
    name: &str,
    get_static: bool,
) -> Option<(String, Option<Span>)> {
    let dynamic = remove_attr_with_span(element, &format!(":{name}"))
        .or_else(|| remove_attr_with_span(element, &format!("v-bind:{name}")));
    if let Some((value, span)) = dynamic {
        Some((parse_filters(&value), span))
    } else if get_static {
        remove_attr_with_span(element, name).map(|(value, span)| (js_string(&value), span))
    } else {
        None
    }
}

fn remove_attr(element: &mut Vue2Element, name: &str) -> Option<String> {
    remove_attr_with_span(element, name).map(|(value, _)| value)
}

fn remove_attr_with_span(element: &mut Vue2Element, name: &str) -> Option<(String, Option<Span>)> {
    let value = element.attrs_map.get(name).cloned()?;
    let span = element.raw_attrs_map.get(name).and_then(|attr| attr.span);
    if let Some(index) = element.attrs_list.iter().position(|attr| attr.name == name) {
        element.attrs_list.remove(index);
    }
    Some((value, span))
}

fn remove_slot_binding(element: &mut Vue2Element) -> Option<(String, String, Option<Span>)> {
    let index = element
        .attrs_list
        .iter()
        .position(|attr| attr.name.starts_with("v-slot") || attr.name.starts_with('#'))?;
    let attr = element.attrs_list.remove(index);
    Some((attr.name, attr.value, attr.span))
}

fn slot_name_from_binding(name: &str) -> (String, bool) {
    let raw = name
        .strip_prefix("v-slot:")
        .or_else(|| name.strip_prefix('#'))
        .unwrap_or("default");
    if raw.starts_with('[') && raw.ends_with(']') {
        (raw[1..raw.len() - 1].to_string(), true)
    } else if raw.is_empty() {
        ("\"default\"".into(), false)
    } else {
        (js_string(raw), false)
    }
}

fn is_directive_name(name: &str) -> bool {
    name.starts_with("v-")
        || name.starts_with('@')
        || name.starts_with(':')
        || name.starts_with('#')
}

fn is_bind_name(name: &str) -> bool {
    name.starts_with(':') || name.starts_with("v-bind:")
}

fn is_on_name(name: &str) -> bool {
    name.starts_with('@') || name.starts_with("v-on:")
}

fn bind_arg_name(name: &str) -> String {
    name.strip_prefix(':')
        .or_else(|| name.strip_prefix("v-bind:"))
        .unwrap_or("")
        .to_string()
}

fn is_dynamic_arg(name: &str) -> bool {
    name.starts_with('[') && name.ends_with(']')
}

fn warn_invalid_dynamic_arg(arg: &str, span: Option<Span>, diagnostics: &mut DiagnosticSink) {
    if arg.contains(char::is_whitespace)
        || arg.contains('\'')
        || arg.contains('"')
        || arg.contains('+')
    {
        diagnostics.push(vue2_warning(
            "W_VUE2_INVALID_DYNAMIC_ARG",
            "Invalid dynamic argument expression: attribute names cannot contain spaces, quotes, <, >, / or =.",
            span,
        ));
    }
}

fn on_arg_name(name: &str) -> String {
    name.strip_prefix('@')
        .or_else(|| name.strip_prefix("v-on:"))
        .unwrap_or("")
        .to_string()
}

fn directive_name_and_arg(raw: &str) -> (String, Option<String>, bool) {
    let raw = raw.strip_prefix("v-").unwrap_or(raw);
    let (name, arg) = raw
        .split_once(':')
        .map_or((raw, None), |(name, arg)| (name, Some(arg.to_string())));
    let is_dynamic = arg
        .as_ref()
        .is_some_and(|arg| arg.starts_with('[') && arg.ends_with(']'));
    let arg = arg.map(|arg| {
        if is_dynamic {
            arg[1..arg.len() - 1].to_string()
        } else {
            arg
        }
    });
    (name.to_string(), arg, is_dynamic)
}

fn split_modifiers(raw_name: &str) -> (String, BTreeMap<String, bool>, Vec<String>) {
    let mut base = String::new();
    let mut modifiers = BTreeMap::new();
    let mut modifier_order = Vec::new();
    let mut in_dynamic = false;
    let mut modifier = String::new();
    let mut reading_modifier = false;
    for ch in raw_name.chars() {
        match ch {
            '[' => {
                in_dynamic = true;
                if reading_modifier {
                    modifier.push(ch);
                } else {
                    base.push(ch);
                }
            }
            ']' => {
                in_dynamic = false;
                if reading_modifier {
                    modifier.push(ch);
                } else {
                    base.push(ch);
                }
            }
            '.' if !in_dynamic => {
                if reading_modifier && !modifier.is_empty() {
                    modifiers.insert(modifier.clone(), true);
                    modifier_order.push(modifier.clone());
                    modifier.clear();
                }
                reading_modifier = true;
            }
            _ if reading_modifier => modifier.push(ch),
            _ => base.push(ch),
        }
    }
    if reading_modifier && !modifier.is_empty() {
        modifiers.insert(modifier.clone(), true);
        modifier_order.push(modifier);
    }
    (base, modifiers, modifier_order)
}

fn normalize_bound_name(name: &str, modifiers: &BTreeMap<String, bool>, dynamic: bool) -> String {
    if dynamic {
        return name.to_string();
    }
    if modifiers.get("prop").copied().unwrap_or(false)
        || modifiers.get("camel").copied().unwrap_or(false)
    {
        let camelized = camelize(name);
        if camelized == "innerHtml" {
            "innerHTML".into()
        } else {
            camelized
        }
    } else {
        name.to_string()
    }
}

fn should_use_prop(
    element: &Vue2Element,
    name: &str,
    modifiers: &BTreeMap<String, bool>,
    options: &Vue2CompileOptions,
) -> bool {
    if modifiers.get("prop").copied().unwrap_or(false) {
        return true;
    }
    if options.disable_default_must_use_prop {
        return false;
    }
    if element.component.is_some() {
        return false;
    }
    if name == "value" && vue2_tag_accepts_value_prop(&element.tag) {
        return element.attrs_map.get("type").map(String::as_str) != Some("button");
    }
    matches!(
        (element.tag.as_str(), name),
        ("option", "selected") | ("input", "checked") | ("video", "muted")
    )
}

fn vue2_tag_accepts_value_prop(tag: &str) -> bool {
    matches!(tag, "input" | "textarea" | "option" | "select" | "progress")
}
