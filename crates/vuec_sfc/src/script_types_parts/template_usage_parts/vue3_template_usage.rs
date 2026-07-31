pub(crate) fn vue27_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue27_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
}

pub(crate) fn vue3_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue3_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
}

pub(crate) fn vue3_template_usage_check_string(template: &str, is_ts: bool) -> String {
    let mut code = String::new();
    let mut tokenizer = HtmlTokenizer::new(template);
    loop {
        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name, attributes, ..
            } => {
                collect_vue3_template_component_usage(&mut code, &name);
                for attribute in attributes {
                    collect_vue3_template_attribute_usage(&mut code, &attribute, is_ts);
                }
            }
            HtmlTokenKind::Text(text) => {
                collect_vue27_template_text_usage(&mut code, &text, is_ts);
            }
            HtmlTokenKind::Eof => break,
            _ => {}
        }
    }
    code.push(';');
    code
}

pub(crate) fn collect_vue3_template_component_usage(code: &mut String, name: &str) {
    let tag = name
        .split_once('.')
        .map(|(base, _)| base.trim())
        .unwrap_or(name);
    if tag.is_empty() || vue3_template_is_builtin_tag(tag) || vue27_template_is_reserved_tag(tag) {
        return;
    }
    let camel = vue27_camelize(tag);
    code.push(',');
    code.push_str(&camel);
    code.push(',');
    code.push_str(&vue27_capitalize(&camel));
}

pub(crate) fn collect_vue3_template_attribute_usage(
    code: &mut String,
    attr: &HtmlAttribute,
    is_ts: bool,
) {
    let name = attr.name.as_str();
    if vue3_template_is_directive_attr(name) {
        let base_name = vue27_template_directive_base_name(name);
        if !vue27_template_is_builtin_dir(&base_name) {
            code.push_str(",v");
            code.push_str(&vue27_capitalize(&vue27_camelize(&base_name)));
        }
        if let Some(arg) = vue3_template_dynamic_argument(name) {
            code.push(',');
            code.push_str(&vue27_process_template_exp(arg, is_ts, None));
        }
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(value, is_ts, Some(&base_name)));
        } else if base_name == "bind" {
            if let Some(arg) = vue3_template_static_bind_argument(name) {
                code.push(',');
                code.push_str(&vue27_camelize(arg));
            }
        }
    } else if name == "ref" {
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(value);
        }
    }
}

pub(crate) fn vue3_template_is_directive_attr(name: &str) -> bool {
    vue27_template_is_directive_attr(name) || name.starts_with('.')
}

pub(crate) fn vue3_template_dynamic_argument(name: &str) -> Option<&str> {
    let start = name.find('[')?;
    let rest = &name[start + 1..];
    let end = rest.find(']')?;
    Some(&rest[..end])
}

pub(crate) fn vue3_template_static_bind_argument(name: &str) -> Option<&str> {
    if vue3_template_dynamic_argument(name).is_some() {
        return None;
    }
    let raw = name
        .strip_prefix(':')
        .or_else(|| name.strip_prefix('.'))
        .or_else(|| name.strip_prefix("v-bind:"))?;
    raw.split('.').next().filter(|arg| !arg.is_empty())
}

pub(crate) fn vue3_template_is_builtin_tag(name: &str) -> bool {
    vue27_template_is_builtin_tag(name)
        || matches!(
            name,
            "Teleport"
                | "teleport"
                | "Suspense"
                | "suspense"
                | "KeepAlive"
                | "keep-alive"
                | "BaseTransition"
                | "base-transition"
                | "Transition"
                | "transition"
                | "TransitionGroup"
                | "transition-group"
        )
}
