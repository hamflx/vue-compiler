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
        .is_some_and(is_identifier_start)
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
        .flat_map(extract_slot_params)
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
