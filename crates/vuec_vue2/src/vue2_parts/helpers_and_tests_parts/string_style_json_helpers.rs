fn condense_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut previous_ws = false;
    for ch in value.chars() {
        if ch.is_ascii_whitespace() {
            if !previous_ws {
                out.push(' ');
            }
            previous_ws = true;
        } else {
            out.push(ch);
            previous_ws = false;
        }
    }
    out
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn js_string_single(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn normalize_vue2_static_class(value: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            pending_space = true;
        } else {
            if !normalized.is_empty() && pending_space {
                normalized.push(' ');
            }
            normalized.push(ch);
            pending_space = false;
        }
    }
    normalized
}

fn vue2_static_style_expression(value: &str) -> String {
    let fields = vue2_parse_static_style(value)
        .into_iter()
        .map(|(name, value)| format!("{}:{}", js_string(&name), js_string(&value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn vue2_parse_static_style(value: &str) -> Vec<(String, String)> {
    let mut style = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    for ch in value.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            ';' if paren_depth == 0 => {
                vue2_push_static_style_decl(&mut style, &current);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    vue2_push_static_style_decl(&mut style, &current);
    style
}

fn vue2_push_static_style_decl(style: &mut Vec<(String, String)>, item: &str) {
    if item.is_empty() {
        return;
    }
    let Some(colon) = item.find(':') else {
        return;
    };
    if colon + 1 >= item.len() {
        return;
    }
    let name = item[..colon].trim();
    let value = item[colon + 1..].trim();
    if name.is_empty() || value.is_empty() {
        return;
    }
    if let Some((_, existing)) = style.iter_mut().find(|(key, _)| key == name) {
        *existing = value.to_string();
    } else {
        style.push((name.to_string(), value.to_string()));
    }
}

fn decode_vue2_attr_entities(
    tag: &str,
    name: &str,
    value: &str,
    options: &Vue2CompileOptions,
) -> String {
    let decode_newlines = (tag == "a" && name == "href" && options.should_decode_newlines_for_href)
        || (!(tag == "a" && name == "href") && options.should_decode_newlines);
    let mut decoded = String::with_capacity(value.len());
    let mut cursor = 0usize;
    while cursor < value.len() {
        let rest = &value[cursor..];
        if let Some((replacement, consumed)) =
            vue2_static_attr_entity_replacement(rest, decode_newlines)
        {
            decoded.push_str(replacement);
            cursor += consumed;
        } else {
            let ch = rest.chars().next().unwrap();
            decoded.push(ch);
            cursor += ch.len_utf8();
        }
    }
    decoded
}

fn vue2_static_attr_entity_replacement(
    value: &str,
    decode_newlines: bool,
) -> Option<(&'static str, usize)> {
    if value.starts_with("&lt;") {
        Some(("<", "&lt;".len()))
    } else if value.starts_with("&gt;") {
        Some((">", "&gt;".len()))
    } else if value.starts_with("&quot;") {
        Some(("\"", "&quot;".len()))
    } else if value.starts_with("&amp;") {
        Some(("&", "&amp;".len()))
    } else if value.starts_with("&#39;") {
        Some(("'", "&#39;".len()))
    } else if decode_newlines && value.starts_with("&#10;") {
        Some(("\n", "&#10;".len()))
    } else if decode_newlines && value.starts_with("&#9;") {
        Some(("\t", "&#9;".len()))
    } else {
        None
    }
}

fn transform_vue2_js_special_newlines(value: &str) -> String {
    value
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn modifiers_json(modifiers: &BTreeMap<String, bool>, raw_name: Option<&str>) -> String {
    let mut keys = Vec::new();
    if let Some(raw_name) = raw_name {
        let (_, _, modifier_order) = split_modifiers(raw_name);
        for key in modifier_order {
            if modifiers.contains_key(&key) && !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    for key in modifiers.keys() {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    let mut array_index_keys = keys
        .iter()
        .filter_map(|key| vue2_js_array_index_key(key).map(|index| (index, key.clone())))
        .collect::<Vec<_>>();
    array_index_keys.sort_by_key(|(index, _)| *index);
    let mut ordered_keys = array_index_keys
        .into_iter()
        .map(|(_, key)| key)
        .collect::<Vec<_>>();
    ordered_keys.extend(
        keys.into_iter()
            .filter(|key| vue2_js_array_index_key(key).is_none()),
    );
    let body = ordered_keys
        .iter()
        .map(|key| format!("{}:true", js_string(key)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

fn vue2_js_array_index_key(key: &str) -> Option<u32> {
    let value = key.parse::<u32>().ok()?;
    (value != u32::MAX && value.to_string() == key).then_some(value)
}

fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| js_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn ast_validators_json(validators: &[vuec_ast::Vue2Validator]) -> String {
    format!(
        "[{}]",
        validators
            .iter()
            .map(|validator| {
                format!(
                    "{{\"name\":{},\"rule\":{}}}",
                    js_string(&validator.name),
                    js_string(&validator.rule)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn ast_validation_result_json(validators: &[vuec_ast::Vue2Validator]) -> String {
    let mut fields = vec!["\"dirty\":false".to_string()];
    fields.extend(
        validators
            .iter()
            .map(|validator| format!("{}:null", js_string(&validator.name))),
    );
    format!("{{{}}}", fields.join(","))
}

fn camelize(value: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '-' {
            uppercase_next = true;
        } else if uppercase_next {
            out.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn hyphenate(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.char_indices() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().collect::<String>();
    out.push_str(chars.as_str());
    out
}
