fn is_simple_path(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    let mut rest = match vue2_simple_path_ident(value) {
        Some(rest) => rest,
        None => return false,
    };
    while !rest.is_empty() {
        if let Some(after_dot) = rest.strip_prefix('.') {
            rest = match vue2_simple_path_ident(after_dot) {
                Some(rest) => rest,
                None => return false,
            };
        } else if let Some(after_bracket) = rest.strip_prefix('[') {
            rest = match vue2_simple_path_bracket(after_bracket) {
                Some(rest) => rest,
                None => return false,
            };
        } else {
            return false;
        }
    }
    true
}

fn vue2_simple_path_ident(value: &str) -> Option<&str> {
    let mut chars = value.char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    for (index, ch) in chars {
        if !is_identifier_continue(ch) {
            return Some(&value[index..]);
        }
    }
    Some("")
}

fn vue2_simple_path_bracket(value: &str) -> Option<&str> {
    if let Some(rest) = value.strip_prefix('\'') {
        let end = rest.find("']")?;
        return Some(&rest[end + 2..]);
    }
    if let Some(rest) = value.strip_prefix('"') {
        let end = rest.find("\"]")?;
        return Some(&rest[end + 2..]);
    }
    let close = value.find(']')?;
    let inner = &value[..close];
    if inner.chars().all(|ch| ch.is_ascii_digit()) || vue2_simple_path_ident(inner) == Some("") {
        Some(&value[close + 1..])
    } else {
        None
    }
}

fn is_function_expression(value: &str) -> bool {
    let value = value.trim_start();
    is_function_keyword_expression(value) || is_arrow_function_expression(value)
}

fn is_function_invocation(value: &str) -> bool {
    let value = value.trim();
    let value = value.trim_end_matches(';');
    if !value.ends_with(')') {
        return false;
    }
    let Some(open) = value.rfind('(') else {
        return false;
    };
    if value[open + 1..value.len() - 1].contains(')') {
        return false;
    }
    is_simple_path(value[..open].trim())
}

fn is_function_keyword_expression(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("function") else {
        return false;
    };
    if rest.starts_with('(') {
        return true;
    }
    if !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let rest = rest.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    let Some((ident, after_ident)) = split_identifier(rest) else {
        return false;
    };
    !ident.is_empty() && after_ident.trim_start().starts_with('(')
}

fn is_arrow_function_expression(value: &str) -> bool {
    let Some(arrow) = value.find("=>") else {
        return false;
    };
    let params = value[..arrow].trim_end();
    if is_simple_identifier(params.trim()) {
        return true;
    }
    params.starts_with('(') && params.ends_with(')') && !params[1..params.len() - 1].contains(')')
}

fn split_identifier(value: &str) -> Option<(&str, &str)> {
    let mut end = 0usize;
    for (index, ch) in value.char_indices() {
        if index == 0 {
            if !is_identifier_start(ch) {
                return None;
            }
        } else if !is_identifier_continue(ch) {
            break;
        }
        end = index + ch.len_utf8();
    }
    (end > 0).then(|| (&value[..end], &value[end..]))
}

fn is_simple_identifier(value: &str) -> bool {
    let Some((ident, rest)) = split_identifier(value) else {
        return false;
    };
    ident.len() == value.len() && rest.is_empty()
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || matches!(ch, '_' | '$')
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')
}
