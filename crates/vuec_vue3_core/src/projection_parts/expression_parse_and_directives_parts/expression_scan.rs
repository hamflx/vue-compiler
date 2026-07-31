#[cfg(test)]
pub(crate) fn process_expression_arrow_body_end(raw: &str, body_start: usize) -> usize {
    if raw[body_start..].starts_with('{') {
        return find_matching_forward(raw, body_start, '{', '}')
            .map(|end| end + 1)
            .unwrap_or(raw.len());
    }
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;
    for (offset, ch) in raw[body_start..].char_indices() {
        let absolute = body_start + offset;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return absolute,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' | ';' if depth == 0 => return absolute,
            _ => {}
        }
    }
    raw.len()
}

pub(crate) fn process_expression_arrow_body_ends(
    raw: &str,
    body_starts: &[usize],
) -> Vec<usize> {
    debug_assert!(body_starts.windows(2).all(|pair| pair[0] < pair[1]));
    debug_assert!(body_starts
        .iter()
        .all(|start| *start <= raw.len() && raw.is_char_boundary(*start)));

    let mut body_ends = vec![raw.len(); body_starts.len()];
    let mut body_depths = vec![0usize; body_starts.len()];
    let mut next_body = 0usize;
    let mut terminators = Vec::<(usize, usize)>::new();
    let mut block_stack = Vec::<Option<usize>>::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;

    for (offset, ch) in raw.char_indices() {
        let mut block_body = None;
        while body_starts
            .get(next_body)
            .is_some_and(|start| *start <= offset)
        {
            body_depths[next_body] = depth;
            if body_starts[next_body] == offset && ch == '{' {
                block_body = Some(next_body);
            }
            next_body += 1;
        }

        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }

        match ch {
            '(' | '[' | '{' => {
                if ch == '{' {
                    block_stack.push(block_body);
                }
                depth = depth.saturating_add(1);
            }
            ')' | ']' | '}' => {
                terminators.push((depth, offset));
                if ch == '}' {
                    if let Some(Some(body)) = block_stack.pop() {
                        body_ends[body] = offset + ch.len_utf8();
                    }
                }
                depth = depth.saturating_sub(1);
            }
            ',' | ';' => {
                terminators.push((depth, offset));
            }
            _ => {}
        }
    }
    while next_body < body_starts.len() {
        body_depths[next_body] = depth;
        next_body += 1;
    }
    terminators.sort_unstable();

    for (index, (&body_start, &body_depth)) in
        body_starts.iter().zip(&body_depths).enumerate()
    {
        if raw[body_start..].starts_with('{') {
            continue;
        }
        let next = terminators.partition_point(|&(depth, offset)| {
            depth < body_depth || depth == body_depth && offset < body_start
        });
        if let Some(&(depth, body_end)) = terminators.get(next) {
            if depth == body_depth {
                body_ends[index] = body_end;
            }
        }
    }
    body_ends
}

pub(crate) fn process_expression_assignment_rhs<'a>(
    raw: &'a str,
    start: usize,
    end: usize,
) -> Option<ProcessExpressionAssignmentRhs<'a>> {
    if !process_expression_assignment_can_start(raw, start) {
        return None;
    }
    let operator_start = skip_ws_forward(raw, end);
    let operator = process_expression_assignment_operator(raw, operator_start)?;
    let rhs_start = skip_ws_forward(raw, operator_start + operator.len());
    let rhs_end = process_expression_assignment_rhs_end(raw, rhs_start);
    let raw_source = raw.get(rhs_start..rhs_end)?;
    let source = raw_source.trim();
    let source_start = rhs_start + raw_source.len().saturating_sub(raw_source.trim_start().len());
    (!source.is_empty()).then_some(ProcessExpressionAssignmentRhs {
        operator,
        source,
        source_start,
    })
}

pub(crate) fn process_expression_assignment_can_start(raw: &str, start: usize) -> bool {
    if previous_non_ws(raw, start).is_none_or(|prev| matches!(prev, '(' | '{' | '[' | ',' | ';')) {
        return true;
    }
    let Some((prev_index, prev)) = previous_non_ws_index(raw, start) else {
        return true;
    };
    if !raw[prev_index + prev.len_utf8()..start]
        .chars()
        .any(is_line_terminator)
    {
        return false;
    }
    process_expression_token_can_end_statement(raw, prev_index, prev)
}

pub(crate) fn process_expression_token_can_end_statement(
    raw: &str,
    index: usize,
    ch: char,
) -> bool {
    ch == ')'
        || ch == ']'
        || ch == '}'
        || ch == '\''
        || ch == '"'
        || ch == '`'
        || is_identifier_continue(ch)
        || ch.is_ascii_digit()
        || raw[..index + ch.len_utf8()].trim_end().ends_with("++")
        || raw[..index + ch.len_utf8()].trim_end().ends_with("--")
}

pub(crate) fn process_expression_assignment_operator(raw: &str, start: usize) -> Option<&str> {
    [
        ">>>=", "<<=", ">>=", "**=", "&&=", "||=", "??=", "+=", "-=", "*=", "/=", "%=", "&=", "|=",
        "^=", "=",
    ]
    .into_iter()
    .find(|operator| raw[start..].starts_with(operator) && !raw[start..].starts_with("=>"))
}

pub(crate) fn process_expression_assignment_rhs_end(raw: &str, rhs_start: usize) -> usize {
    process_expression_assignment_rhs_end_ignoring_ranges(raw, rhs_start, &[])
}

pub(crate) fn process_expression_assignment_rhs_end_ignoring_ranges(
    raw: &str,
    rhs_start: usize,
    ignored_ranges: &[(usize, usize)],
) -> usize {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut ignored_range_index = ignored_ranges.partition_point(|(_, end)| *end <= rhs_start);
    let mut chars = raw[rhs_start..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        let absolute = rhs_start + offset;
        while ignored_ranges
            .get(ignored_range_index)
            .is_some_and(|(_, end)| *end <= absolute)
        {
            ignored_range_index += 1;
        }
        if let Some(&(start, end)) = ignored_ranges.get(ignored_range_index) {
            if start <= absolute && absolute < end {
                while chars
                    .peek()
                    .is_some_and(|(offset, _)| rhs_start + *offset < end)
                {
                    chars.next();
                }
                ignored_range_index += 1;
                continue;
            }
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return raw[..absolute].trim_end().len(),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' | ';' if depth == 0 => return raw[..absolute].trim_end().len(),
            '\n' | '\r'
                if depth == 0
                    && process_expression_line_terminator_can_end_rhs(raw, rhs_start, absolute) =>
            {
                return raw[..absolute].trim_end().len();
            }
            _ => {}
        }
    }
    raw.trim_end().len()
}

pub(crate) fn process_expression_line_terminator_can_end_rhs(
    raw: &str,
    rhs_start: usize,
    offset: usize,
) -> bool {
    if raw[rhs_start..offset].trim().is_empty() {
        return false;
    }
    let mut next_offset = offset;
    while next_offset < raw.len() {
        let Some(ch) = raw[next_offset..].chars().next() else {
            break;
        };
        if !is_line_terminator(ch) {
            break;
        }
        next_offset += ch.len_utf8();
    }
    !next_non_ws(raw, next_offset).is_some_and(process_expression_token_continues_expression)
}

pub(crate) fn process_expression_token_continues_expression(ch: char) -> bool {
    matches!(
        ch,
        '.' | '?'
            | ':'
            | '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '&'
            | '|'
            | '^'
            | '='
            | '<'
            | '>'
            | '('
            | '['
    )
}

pub(crate) fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r')
}

pub(crate) fn process_expression_is_destructure_assignment(raw: &str, start: usize) -> bool {
    let Some(open) = process_expression_destructure_open_before(raw, start) else {
        return false;
    };
    let close_ch = match raw.as_bytes().get(open) {
        Some(b'{') => '}',
        Some(b'[') => ']',
        _ => return false,
    };
    let Some(close) = find_matching_forward(raw, open, raw.as_bytes()[open] as char, close_ch)
    else {
        return false;
    };
    if !(open < start && start < close) {
        return false;
    }
    next_non_ws(raw, close + close_ch.len_utf8()) == Some('=')
}

pub(crate) fn process_expression_may_have_destructure_assignment(raw: &str) -> bool {
    raw.as_bytes().contains(&b'=')
        && raw
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'{' | b'['))
}

pub(crate) fn process_expression_is_static_member(raw: &str, start: usize) -> bool {
    let prefix = raw[..start].trim_end();
    prefix.ends_with('.') && !prefix.ends_with("...")
}

pub(crate) fn process_expression_destructure_open_before(raw: &str, start: usize) -> Option<usize> {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut stack = Vec::<(usize, char)>::new();
    for (offset, ch) in raw[..start].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => stack.push((offset, ch)),
            ')' => pop_matching_open(&mut stack, '('),
            ']' => pop_matching_open(&mut stack, '['),
            '}' => pop_matching_open(&mut stack, '{'),
            _ => {}
        }
    }
    stack
        .into_iter()
        .rev()
        .find_map(|(offset, ch)| matches!(ch, '{' | '[').then_some(offset))
}

pub(crate) fn pop_matching_open(stack: &mut Vec<(usize, char)>, expected: char) {
    if stack.last().is_some_and(|(_, ch)| *ch == expected) {
        stack.pop();
    }
}

pub(crate) fn process_expression_dynamic_static_reference(
    raw: &str,
    start: usize,
    end: usize,
) -> bool {
    next_non_ws(raw, end) == Some('(') || process_expression_preceded_by_new(raw, start)
}

pub(crate) fn process_expression_preceded_by_new(raw: &str, start: usize) -> bool {
    let prefix = raw[..start].trim_end();
    prefix.strip_suffix("new").is_some_and(|before| {
        before
            .chars()
            .last()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

pub(crate) fn skip_ws_forward(raw: &str, mut offset: usize) -> usize {
    while offset < raw.len() {
        let Some(ch) = raw[offset..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        offset += ch.len_utf8();
    }
    offset
}

pub(crate) fn previous_non_ws_index(source: &str, offset: usize) -> Option<(usize, char)> {
    source[..offset]
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
}

pub(crate) fn previous_char(source: &str, offset: usize) -> Option<(usize, char)> {
    source[..offset].char_indices().next_back()
}

pub(crate) fn find_matching_forward(
    raw: &str,
    open: usize,
    open_ch: char,
    close_ch: char,
) -> Option<usize> {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut depth = 0usize;
    for (offset, ch) in raw[open..].char_indices() {
        let absolute = open + offset;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        if ch == open_ch {
            depth += 1;
        } else if ch == close_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(absolute);
            }
        }
    }
    None
}

pub(crate) fn find_matching_backward(
    raw: &str,
    close: usize,
    open_ch: char,
    close_ch: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in raw[..=close].char_indices().rev() {
        if ch == close_ch {
            depth += 1;
        } else if ch == open_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

pub(crate) fn process_expression_update_argument(
    raw: &str,
    start: usize,
    end: usize,
) -> Option<ProcessExpressionUpdate> {
    if let Some(tail) = raw.get(end..).map(str::trim_start) {
        if tail.starts_with("++") {
            return Some(ProcessExpressionUpdate {
                operator: "++",
                prefix: false,
            });
        }
        if tail.starts_with("--") {
            return Some(ProcessExpressionUpdate {
                operator: "--",
                prefix: false,
            });
        }
    }
    if let Some(head) = raw.get(..start).map(str::trim_end) {
        if head.ends_with("++") {
            return Some(ProcessExpressionUpdate {
                operator: "++",
                prefix: true,
            });
        }
        if head.ends_with("--") {
            return Some(ProcessExpressionUpdate {
                operator: "--",
                prefix: true,
            });
        }
    }
    None
}
