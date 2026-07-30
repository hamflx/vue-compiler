#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
    BlockComment,
}

pub(crate) fn collect_scoped_keyframes(
    source: &str,
    short_id: &str,
    budget: &mut ScopedStyleBudget,
) -> Result<BTreeMap<String, String>, StylePreprocessError> {
    let mut keyframes = BTreeMap::new();
    collect_scoped_keyframes_in(source, short_id, &mut keyframes, budget)?;
    Ok(keyframes)
}

pub(crate) fn collect_scoped_keyframes_in(
    source: &str,
    short_id: &str,
    keyframes: &mut BTreeMap<String, String>,
    budget: &mut ScopedStyleBudget,
) -> Result<(), StylePreprocessError> {
    let mut cursor = 0usize;
    while cursor < source.len() {
        cursor = skip_css_whitespace(source, cursor);
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let Some(close) = find_matching_brace(source, delimiter) else {
            break;
        };
        let prelude = source[cursor..delimiter].trim();
        if let Some((name, params)) = parse_at_rule(prelude) {
            if is_keyframes_name(name) && !scoped_keyframe_name_has_suffix(params, short_id) {
                if !keyframes.contains_key(params) {
                    let renamed_bytes = params
                        .len()
                        .checked_add(1)
                        .and_then(|bytes| bytes.checked_add(short_id.len()))
                        .ok_or_else(|| {
                            StylePreprocessError::scoped_limit(
                                "scoped style keyframe name size overflowed",
                            )
                        })?;
                    budget.claim_keyframe(params.len(), renamed_bytes)?;
                    let raw = copy_scoped_keyframe_name(params)?;
                    let mut renamed = String::new();
                    renamed.try_reserve_exact(renamed_bytes).map_err(|_| {
                        StylePreprocessError::scoped_limit(
                            "scoped style keyframe name could not reserve capacity within the configured limit",
                        )
                    })?;
                    renamed.push_str(params);
                    renamed.push('-');
                    renamed.push_str(short_id);
                    keyframes.insert(raw, renamed);
                }
            } else {
                collect_scoped_keyframes_in(
                    &source[delimiter + 1..close],
                    short_id,
                    keyframes,
                    budget,
                )?;
            }
        } else {
            collect_scoped_keyframes_in(
                &source[delimiter + 1..close],
                short_id,
                keyframes,
                budget,
            )?;
        }
        cursor = close + 1;
    }
    Ok(())
}

fn copy_scoped_keyframe_name(name: &str) -> Result<String, StylePreprocessError> {
    let mut output = String::new();
    output.try_reserve_exact(name.len()).map_err(|_| {
        StylePreprocessError::scoped_limit(
            "scoped style keyframe name could not reserve capacity within the configured limit",
        )
    })?;
    output.push_str(name);
    Ok(output)
}

pub(crate) fn scoped_keyframe_name_has_suffix(name: &str, short_id: &str) -> bool {
    name.strip_suffix(short_id)
        .is_some_and(|prefix| prefix.ends_with('-'))
}

pub(crate) fn rewrite_at_rule_prelude(
    prelude: &str,
    keyframes: &BTreeMap<String, String>,
) -> String {
    let Some((name, params)) = parse_at_rule(prelude) else {
        return prelude.to_string();
    };
    if !is_keyframes_name(name) {
        return prelude.to_string();
    }
    let Some(renamed) = lookup_keyframe_name(params, keyframes) else {
        return prelude.to_string();
    };
    format!("@{name} {renamed}")
}

pub(crate) fn parse_at_rule(prelude: &str) -> Option<(&str, &str)> {
    let prelude = prelude.trim();
    let rest = prelude.strip_prefix('@')?;
    let name_end = rest
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(rest.len());
    Some((&rest[..name_end], rest[name_end..].trim()))
}

pub(crate) fn is_keyframes_at_rule(prelude: &str) -> bool {
    parse_at_rule(prelude)
        .map(|(name, _)| is_keyframes_name(name))
        .unwrap_or(false)
}

pub(crate) fn is_keyframes_name(name: &str) -> bool {
    name.ends_with("keyframes")
}

pub(crate) fn lookup_keyframe_name<'a>(
    name: &str,
    keyframes: &'a BTreeMap<String, String>,
) -> Option<&'a String> {
    keyframes.get(name)
}

pub(crate) fn rewrite_animation_declarations(
    source: &str,
    keyframes: &BTreeMap<String, String>,
) -> String {
    if keyframes.is_empty() {
        return source.to_string();
    }

    let mut output = String::new();
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(source) {
        output.push_str(&rewrite_declaration_segment(
            &source[segment_start..semicolon],
            keyframes,
        ));
        output.push(';');
        segment_start = semicolon + 1;
    }
    output.push_str(&rewrite_declaration_segment(
        &source[segment_start..],
        keyframes,
    ));
    output
}

pub(crate) fn top_level_semicolons(source: &str) -> Vec<usize> {
    let mut semicolons = Vec::new();
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    ';' if paren_depth == 0 => semicolons.push(index),
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    semicolons
}

pub(crate) fn rewrite_declaration_segment(
    segment: &str,
    keyframes: &BTreeMap<String, String>,
) -> String {
    let Some(colon) = find_top_level_colon(segment) else {
        return segment.to_string();
    };
    let prop = segment[..colon].trim();
    let value_start = colon + 1;
    let value = &segment[value_start..];
    let leading_value_whitespace = value
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(value.len());
    let value_prefix = &value[..leading_value_whitespace];
    let value_body = &value[leading_value_whitespace..];
    let rewritten = if is_animation_name_property(prop) {
        rewrite_animation_name_value(value_body.trim(), keyframes)
    } else if is_animation_property(prop) {
        rewrite_animation_value(value_body.trim(), keyframes)
    } else {
        return segment.to_string();
    };

    let mut output = String::new();
    output.push_str(&segment[..value_start]);
    output.push_str(value_prefix);
    output.push_str(&rewritten);
    output
}

pub(crate) fn find_top_level_colon(source: &str) -> Option<usize> {
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    ':' if paren_depth == 0 => return Some(index),
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn is_animation_name_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation-name" || (prop.starts_with('-') && prop.ends_with("-animation-name"))
}

pub(crate) fn is_animation_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation" || (prop.starts_with('-') && prop.ends_with("-animation"))
}

pub(crate) fn rewrite_animation_name_value(
    value: &str,
    keyframes: &BTreeMap<String, String>,
) -> String {
    value
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            lookup_keyframe_name(trimmed, keyframes)
                .cloned()
                .unwrap_or_else(|| trimmed.to_string())
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn rewrite_animation_value(
    value: &str,
    keyframes: &BTreeMap<String, String>,
) -> String {
    value
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            let mut values = trimmed.split_whitespace().collect::<Vec<_>>();
            let Some((index, rewritten)) = values
                .iter()
                .enumerate()
                .find_map(|(index, value)| {
                    lookup_keyframe_name(value, keyframes).map(|rewritten| (index, rewritten))
                })
            else {
                return part.to_string();
            };
            values[index] = rewritten.as_str();
            values.join(" ")
        })
        .collect::<Vec<_>>()
        .join(",")
}
