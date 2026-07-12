#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForParsed {
    pub(crate) source: Vue3ForPart,
    pub(crate) value: Option<Vue3ForPart>,
    pub(crate) key: Option<Vue3ForPart>,
    pub(crate) index: Option<Vue3ForPart>,
}

impl Vue3ForParsed {
    pub(crate) fn all_alias_locals(&self) -> Vec<String> {
        let mut locals = Vec::new();
        for part in [&self.value, &self.key, &self.index].into_iter().flatten() {
            for local in vue3_for_alias_locals(&part.content) {
                if !locals.iter().any(|existing| existing == &local) {
                    locals.push(local);
                }
            }
        }
        locals
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForPart {
    pub(crate) content: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3ForAstMode {
    Expression,
    Params,
}

pub(crate) fn parse_vue3_for_expression(source: &str) -> Option<Vue3ForParsed> {
    let Vue3ForMatch { lhs_end, rhs_start } = find_vue3_for_match(source)?;
    let rhs_end = trim_end_offset(source, rhs_start, source.len());
    if rhs_start >= rhs_end {
        return None;
    }
    let (alias_start, alias_end) = vue3_for_alias_content_span(source, 0, lhs_end);
    let aliases = split_vue3_for_aliases(source, alias_start, alias_end);
    Some(Vue3ForParsed {
        source: Vue3ForPart {
            content: source[rhs_start..rhs_end].to_string(),
            start: rhs_start,
            end: rhs_end,
        },
        value: aliases.first().and_then(|segment| segment.part(source)),
        key: aliases.get(1).and_then(|segment| segment.part(source)),
        index: aliases.get(2).and_then(|segment| segment.part(source)),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForMatch {
    pub(crate) lhs_end: usize,
    pub(crate) rhs_start: usize,
}

pub(crate) fn find_vue3_for_match(source: &str) -> Option<Vue3ForMatch> {
    for (operator_start, _) in source.char_indices() {
        let operator_len = if source[operator_start..].starts_with("in")
            || source[operator_start..].starts_with("of")
        {
            2
        } else {
            continue;
        };
        if operator_start == 0 || !previous_char_is_whitespace(source, operator_start) {
            continue;
        }
        let after_operator = operator_start + operator_len;
        if !source[after_operator..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            continue;
        }
        let Some(rhs_start) = source[after_operator..]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(index, _)| after_operator + index)
        else {
            continue;
        };
        let lhs_end = trim_end_offset(source, 0, operator_start);
        return Some(Vue3ForMatch { lhs_end, rhs_start });
    }
    None
}

pub(crate) fn previous_char_is_whitespace(source: &str, offset: usize) -> bool {
    source[..offset]
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
}

pub(crate) fn vue3_for_alias_content_span(
    source: &str,
    start: usize,
    end: usize,
) -> (usize, usize) {
    let mut start = trim_start_offset(source, start, end);
    let mut end = trim_end_offset(source, start, end);
    if source[start..end].starts_with('(') && source[start..end].ends_with(')') {
        start += '('.len_utf8();
        end = end.saturating_sub(')'.len_utf8());
    }
    start = trim_start_offset(source, start, end);
    end = trim_end_offset(source, start, end);
    (start, end)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ForAliasSegment {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Vue3ForAliasSegment {
    pub(crate) fn part(&self, source: &str) -> Option<Vue3ForPart> {
        let start = trim_start_offset(source, self.start, self.end);
        let end = trim_end_offset(source, start, self.end);
        (start < end).then(|| Vue3ForPart {
            content: source[start..end].to_string(),
            start,
            end,
        })
    }
}

pub(crate) fn split_vue3_for_aliases(
    source: &str,
    alias_start: usize,
    alias_end: usize,
) -> Vec<Vue3ForAliasSegment> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut quote = None::<char>;
    let mut start = alias_start;
    let mut escaped = false;
    for (index, ch) in source[alias_start..alias_end].char_indices() {
        let index = alias_start + index;
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
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(Vue3ForAliasSegment { start, end: index });
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    items.push(Vue3ForAliasSegment {
        start,
        end: alias_end,
    });
    items
}

pub(crate) fn trim_start_offset(source: &str, start: usize, end: usize) -> usize {
    source[start..end]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| start + index)
        .unwrap_or(end)
}

pub(crate) fn trim_end_offset(source: &str, start: usize, end: usize) -> usize {
    let mut trimmed = end;
    for (index, ch) in source[start..end].char_indices().rev() {
        if !ch.is_whitespace() {
            trimmed = start + index + ch.len_utf8();
            break;
        }
        trimmed = start + index;
    }
    trimmed
}
