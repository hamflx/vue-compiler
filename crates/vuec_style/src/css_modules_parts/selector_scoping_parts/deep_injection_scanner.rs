#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeepCombinator {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn find_deep_combinator(selector: &str) -> Option<DeepCombinator> {
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                _ if paren_depth == 0 && bracket_depth == 0 => {
                    if selector[index..].starts_with(">>>") {
                        return Some(DeepCombinator {
                            start: index,
                            end: index + 3,
                        });
                    }
                    if selector[index..].starts_with("/deep/") {
                        return Some(DeepCombinator {
                            start: index,
                            end: index + "/deep/".len(),
                        });
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn rewrite_deep_selector(prefix: &str, suffix: &str, scope_id: &str) -> String {
    let scoped = inject_scope_attribute(prefix.trim_end(), scope_id);
    let suffix = suffix.trim_start();
    if suffix.is_empty() {
        scoped
    } else {
        format!("{scoped} {suffix}")
    }
}

pub(crate) fn rewrite_deep_combinator_selector_without_scope(selector: &str) -> Option<String> {
    let deep = find_deep_combinator(selector)?;
    let prefix = selector[..deep.start].trim_end();
    let suffix = selector[deep.end..].trim_start();
    if suffix.is_empty() {
        Some(prefix.to_string())
    } else if prefix.is_empty() {
        Some(suffix.to_string())
    } else {
        Some(format!("{prefix} {suffix}"))
    }
}

pub(crate) fn inject_scope_after_container_pseudo(
    selector: &str,
    name: &str,
    scope_id: &str,
) -> String {
    let Some(container) = find_top_level_pseudo_function(selector, &[name]) else {
        return inject_scope_attribute(selector, scope_id);
    };
    let index = container.end;
    let mut rewritten = String::new();
    rewritten.push_str(&selector[..index]);
    rewritten.push('[');
    rewritten.push_str(scope_id);
    rewritten.push(']');
    rewritten.push_str(&selector[index..]);
    rewritten
}

pub(crate) fn inject_scope_attribute(selector: &str, scope_id: &str) -> String {
    let selector = strip_leading_universal_selector(selector.trim());
    let Some(index) = selector_injection_index(selector) else {
        return format!("[{scope_id}]{selector}");
    };
    let mut rewritten = String::new();
    let mut prefix_end = index;
    let mut removed_universal = false;
    if let Some(stripped) = selector[..index].strip_suffix('*') {
        if selector[index..].starts_with(['.', '#', ':', '[']) {
            prefix_end = stripped.len();
            removed_universal = true;
        }
    }
    if removed_universal {
        rewritten.push_str(&selector[..prefix_end]);
    } else {
        rewritten.push_str(selector[..prefix_end].trim_end());
    }
    rewritten.push('[');
    rewritten.push_str(scope_id);
    rewritten.push(']');
    rewritten.push_str(&selector[index..]);
    rewritten
}

pub(crate) fn strip_leading_universal_selector(selector: &str) -> &str {
    let Some(after_star) = selector.strip_prefix('*') else {
        return selector;
    };
    if after_star.is_empty() {
        return "";
    }
    if let Some(first) = after_star.chars().next() {
        if !first.is_whitespace() {
            return after_star;
        }
    }
    let whitespace_end = skip_selector_whitespace(selector, '*'.len_utf8());
    if whitespace_end >= selector.len() {
        return "";
    }
    let next = selector[whitespace_end..].chars().next();
    if next.is_some_and(|ch| {
        ch == '.'
            || ch == '#'
            || ch == '['
            || ch == ':'
            || ch == '\\'
            || is_selector_ident_start(ch)
    }) {
        &selector[whitespace_end..]
    } else {
        after_star
    }
}

pub(crate) fn selector_injection_index(selector: &str) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut last_node_end = None;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    let end = consume_selector_token(selector, index);
                    last_node_end = Some(end);
                    index = end;
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    let end = skip_selector_comment(selector, index);
                    last_node_end = Some(end);
                    index = end;
                    continue;
                }
                '[' => {
                    let Some(end) = find_matching_selector_bracket(selector, index) else {
                        return last_node_end.or(Some(selector.len()));
                    };
                    last_node_end = Some(end + 1);
                    index = end + 1;
                    continue;
                }
                ':' => {
                    let end = skip_selector_pseudo(selector, index);
                    index = end;
                    continue;
                }
                '(' => {
                    if let Some(close) = find_matching_selector_paren(selector, index) {
                        last_node_end = Some(close + 1);
                        index = close + 1;
                        continue;
                    }
                }
                '>' | '+' | '~' | ',' => {}
                '*' if last_node_end.is_none() => last_node_end = Some(index + ch.len_utf8()),
                '*' => {}
                _ if ch.is_whitespace() => {}
                '&' => {
                    last_node_end = Some(index + ch.len_utf8());
                }
                _ if is_selector_ident_start(ch) || ch == '.' || ch == '#' => {
                    let end = consume_selector_token(selector, index);
                    last_node_end = Some(end);
                    index = end;
                    continue;
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    last_node_end
}

pub(crate) fn find_matching_selector_bracket(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut index = open + 1;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                ']' => return Some(index),
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn skip_selector_pseudo(selector: &str, start: usize) -> usize {
    let mut index = start;
    if selector[index..].starts_with("::") {
        index += 2;
    } else {
        index += 1;
    }
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            break;
        }
        index += ch.len_utf8();
    }
    let open = skip_selector_whitespace(selector, index);
    if open < selector.len() && selector[open..].starts_with('(') {
        if let Some(close) = find_matching_selector_paren(selector, open) {
            return close + 1;
        }
    }
    index
}

pub(crate) fn consume_selector_token(selector: &str, start: usize) -> usize {
    let mut index = start;
    if selector[index..].starts_with('.') || selector[index..].starts_with('#') {
        index += 1;
    }
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if ch == '\\' {
            index = consume_selector_escape(selector, index);
            continue;
        }
        if !is_selector_ident_continue(ch) {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

pub(crate) fn is_selector_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '-' || !ch.is_ascii()
}

pub(crate) fn is_selector_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || !ch.is_ascii()
}

pub(crate) fn normalize_selector_comments(selector: &str) -> String {
    let mut output = String::with_capacity(selector.len());
    let mut state = SelectorScannerState::Normal;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => {
                    state = SelectorScannerState::SingleQuote;
                    output.push(ch);
                    index += ch.len_utf8();
                }
                '"' => {
                    state = SelectorScannerState::DoubleQuote;
                    output.push(ch);
                    index += ch.len_utf8();
                }
                '/' if selector[index..].starts_with("/*") => {
                    let end = skip_selector_comment(selector, index);
                    let before_is_whitespace =
                        output.chars().next_back().is_some_and(char::is_whitespace);
                    let after_is_whitespace = if end < selector.len() {
                        selector[end..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                    } else {
                        false
                    };
                    if !before_is_whitespace && !after_is_whitespace {
                        output.push_str(&selector[index..end]);
                    }
                    index = end;
                }
                _ => {
                    output.push(ch);
                    index += ch.len_utf8();
                }
            },
            SelectorScannerState::SingleQuote => {
                output.push(ch);
                index += ch.len_utf8();
                if ch == '\\' {
                    if index < selector.len() {
                        if let Some(next) = selector[index..].chars().next() {
                            output.push(next);
                            index += next.len_utf8();
                        }
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                output.push(ch);
                index += ch.len_utf8();
                if ch == '\\' {
                    if index < selector.len() {
                        if let Some(next) = selector[index..].chars().next() {
                            output.push(next);
                            index += next.len_utf8();
                        }
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
    }
    output
}

pub(crate) fn skip_selector_comment(selector: &str, start: usize) -> usize {
    selector[start + 2..]
        .find("*/")
        .map(|offset| start + 2 + offset + 2)
        .unwrap_or(selector.len())
}

pub(crate) fn consume_selector_escape(selector: &str, start: usize) -> usize {
    let mut index = start + '\\'.len_utf8();
    if index >= selector.len() {
        return index;
    }

    let mut hex_digits = 0usize;
    while index < selector.len() && hex_digits < 6 {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !ch.is_ascii_hexdigit() {
            break;
        }
        index += ch.len_utf8();
        hex_digits += 1;
    }

    if hex_digits > 0 {
        if index < selector.len() {
            if let Some(ch) = selector[index..].chars().next() {
                if ch.is_whitespace() {
                    index += ch.len_utf8();
                }
            }
        }
        return index;
    }

    index + selector[index..].chars().next().map_or(0, char::len_utf8)
}
