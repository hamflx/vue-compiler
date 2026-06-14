/// Parses a Vue 2 filter chain without treating filter pipes as JavaScript syntax.
pub fn parse_vue2_filter_expression(source_text: &str) -> Vue2FilterExpression<'_> {
    let mut filters = Vec::new();
    let mut expression: Option<&str> = None;
    let mut last_filter_index = 0usize;
    let mut state = JsScanState::default();
    let mut prev = '\0';

    for (index, ch) in source_text.char_indices() {
        if state.consume(source_text, index, ch, prev) {
            prev = ch;
            continue;
        }
        if ch == '|'
            && source_text[index + ch.len_utf8()..].chars().next() != Some('|')
            && prev != '|'
            && state.depth_is_zero()
        {
            if expression.is_none() {
                expression = Some(source_text[..index].trim());
                last_filter_index = index + ch.len_utf8();
            } else {
                let raw = source_text[last_filter_index..index].trim();
                if !raw.is_empty() {
                    filters.push(parse_vue2_filter_call(raw));
                }
                last_filter_index = index + ch.len_utf8();
            }
        }
        prev = ch;
    }

    let base = if let Some(expression) = expression {
        let raw = source_text[last_filter_index..].trim();
        if !raw.is_empty() {
            filters.push(parse_vue2_filter_call(raw));
        }
        expression
    } else {
        source_text.trim()
    };

    Vue2FilterExpression {
        raw: source_text,
        base,
        filters,
    }
}

/// Converts a Vue 2 filter chain into nested `_f()` runtime helper calls.
pub fn rewrite_vue2_filter_expression(source_text: &str) -> String {
    let parsed = parse_vue2_filter_expression(source_text);
    let mut expression = parsed.base.to_string();
    for filter in parsed.filters {
        expression = wrap_vue2_filter(&expression, &filter);
    }
    expression
}

/// Result of parsing source through a selected [`JsParseMode`].
pub enum JsParseResult<'a> {
    /// Parsed program result.
    Program(ParserReturn<'a>),
    /// Parsed expression node.
    Expression(Expression<'a>),
    /// Parsed parameter list.
    Params(ParsedParams<'a>),
    /// Parsed Vue `v-for` expression.
    ForExpression(ParsedForExpression<'a>),
}

fn split_for_expression(source_text: &str) -> Option<(&str, &str)> {
    let mut index = 0usize;
    let mut state = JsScanState::default();
    let mut prev = '\0';
    while index < source_text.len() {
        let ch = source_text[index..].chars().next()?;
        if state.consume(source_text, index, ch, prev) {
            prev = ch;
            index += ch.len_utf8();
            continue;
        }
        if ch == ' ' && state.depth_is_zero() {
            let rest = &source_text[index..];
            if rest.starts_with(" in ") {
                let left = source_text[..index].trim();
                let right = source_text[index + 4..].trim();
                return Some((left, right));
            }
            if rest.starts_with(" of ") {
                let left = source_text[..index].trim();
                let right = source_text[index + 4..].trim();
                return Some((left, right));
            }
        }
        prev = ch;
        index += ch.len_utf8();
    }
    None
}

fn split_top_level(source_text: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut state = JsScanState::default();
    let mut prev = '\0';
    let mut start = 0usize;
    for (index, ch) in source_text.char_indices() {
        if state.consume(source_text, index, ch, prev) {
            prev = ch;
            continue;
        }
        if ch == separator && state.depth_is_zero() {
            let item = source_text[start..index].trim();
            if !item.is_empty() {
                items.push(item);
            }
            start = index + ch.len_utf8();
        }
        prev = ch;
    }
    let tail = source_text[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn primary_label_offset(diagnostic: &OxcDiagnostic) -> Option<usize> {
    let labels = diagnostic.labels.as_ref()?;
    labels
        .iter()
        .find(|label| label.primary())
        .or_else(|| labels.first())
        .map(LabeledSpan::offset)
}

fn parse_oxc_line_column(message: &str) -> Option<(usize, usize)> {
    for line in message.lines() {
        let trimmed = line.trim();
        let Some(open) = trimmed.rfind('(') else {
            continue;
        };
        let Some(close) = trimmed[open + 1..].find(')') else {
            continue;
        };
        let location = &trimmed[open + 1..open + 1 + close];
        let Some((line, column)) = location.split_once(':') else {
            continue;
        };
        let line = line.parse::<usize>().ok()?;
        let column = column.parse::<usize>().ok()?;
        return Some((line, column));
    }
    None
}

fn parse_vue2_filter_call(raw: &str) -> Vue2FilterCall<'_> {
    if let Some(open) = filter_call_open_paren(raw) {
        let name = raw[..open].trim();
        let close = raw.rfind(')').unwrap_or(raw.len());
        let args_source = raw[open + 1..close].trim();
        Vue2FilterCall {
            name,
            args: split_top_level(args_source, ','),
            raw,
        }
    } else {
        Vue2FilterCall {
            name: raw.trim(),
            args: Vec::new(),
            raw,
        }
    }
}

fn filter_call_open_paren(raw: &str) -> Option<usize> {
    let mut state = JsScanState::default();
    let mut prev = '\0';
    for (index, ch) in raw.char_indices() {
        let top_level = state.depth_is_zero();
        if state.consume(raw, index, ch, prev) {
            prev = ch;
            continue;
        }
        if ch == '(' && top_level {
            return Some(index);
        }
        prev = ch;
    }
    None
}

fn wrap_vue2_filter(exp: &str, filter: &Vue2FilterCall<'_>) -> String {
    if let Some(open) = filter_call_open_paren(filter.raw) {
        let args = &filter.raw[open + 1..];
        if args == ")" {
            format!("_f(\"{}\")({exp})", filter.name)
        } else {
            format!("_f(\"{}\")({exp},{args}", filter.name)
        }
    } else {
        format!("_f(\"{}\")({exp})", filter.name)
    }
}
