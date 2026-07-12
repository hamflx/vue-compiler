pub(crate) fn rewrite_js_like_expression(
    expression: &str,
    options: &Vue3CompilerOptions,
) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, Vec::new(), &mut output);
    output
}

pub(crate) fn rewrite_js_like_expression_with_locals(
    expression: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, locals.to_vec(), &mut output);
    output
}

pub(crate) fn rewrite_js_like_expression_into(
    expression: &str,
    options: &Vue3CompilerOptions,
    root_locals: Vec<String>,
    output: &mut String,
) {
    let regular_expression_ranges = js_like_regular_expression_ranges(expression, options);
    rewrite_js_like_expression_into_with_ranges(
        expression,
        options,
        root_locals,
        output,
        &regular_expression_ranges,
    );
}

fn rewrite_js_like_expression_into_with_ranges(
    expression: &str,
    options: &Vue3CompilerOptions,
    root_locals: Vec<String>,
    output: &mut String,
    regular_expression_ranges: &[(usize, usize)],
) {
    let mut regular_expression_index = 0usize;
    let mut scopes = vec![Scope {
        locals: root_locals,
    }];
    let mut previous = TokenKind::Other;
    let mut pending_decl: Option<DeclKind> = None;
    let mut pending_function_params = false;
    let mut last_keyword: Option<String> = None;
    let mut paren_depth = 0usize;
    let mut for_pending = false;
    let mut for_header_depth: Option<usize> = None;
    let mut pending_for_block_locals = Vec::<String>::new();
    let mut catch_pending = false;
    let mut catch_param_depth: Option<usize> = None;
    let mut pending_catch_locals = Vec::<String>::new();
    let chars = expression.char_indices().collect::<Vec<_>>();
    let arrow_bindings = process_expression_arrow_bindings(expression);
    let mut index = 0usize;
    while index < chars.len() {
        let byte = chars[index].0;
        let ch = chars[index].1;
        while regular_expression_ranges
            .get(regular_expression_index)
            .is_some_and(|(_, end)| *end <= byte)
        {
            regular_expression_index += 1;
        }
        if ch == '/' {
            if let Some(tail) = expression.get(byte..) {
                if tail.starts_with("//") {
                    while index < chars.len() {
                        let current = chars[index].1;
                        output.push(current);
                        index += 1;
                        if current == '\n' || current == '\r' {
                            break;
                        }
                    }
                    previous = TokenKind::Other;
                    continue;
                }
                if tail.starts_with("/*") {
                    output.push('/');
                    output.push('*');
                    index += 2;
                    while index < chars.len() {
                        let current = chars[index].1;
                        output.push(current);
                        index += 1;
                        if current == '*' && index < chars.len() && chars[index].1 == '/' {
                            output.push('/');
                            index += 1;
                            break;
                        }
                    }
                    previous = TokenKind::Other;
                    continue;
                }
            }
            if let Some(&(start, end)) = regular_expression_ranges.get(regular_expression_index) {
                if start == byte {
                    output.push_str(&expression[start..end]);
                    regular_expression_index += 1;
                    while index < chars.len() && chars[index].0 < end {
                        index += 1;
                    }
                    previous = TokenKind::Other;
                    continue;
                }
            }
        }
        if ch == '`' {
            index = rewrite_template_literal_into(
                expression,
                &chars,
                index,
                options,
                &scopes,
                output,
                regular_expression_ranges,
            );
            previous = TokenKind::Other;
            continue;
        }
        if ch == '\'' || ch == '"' {
            let quote = ch;
            output.push(ch);
            index += 1;
            while index < chars.len() {
                let current = chars[index].1;
                output.push(current);
                index += 1;
                if current == '\\' && index < chars.len() {
                    output.push(chars[index].1);
                    index += 1;
                    continue;
                }
                if current == quote {
                    break;
                }
            }
            previous = TokenKind::Other;
            continue;
        }
        if (ch == '+' || ch == '-')
            && expression
                .get(byte..)
                .is_some_and(|tail| tail.starts_with("++") || tail.starts_with("--"))
        {
            let operator = if ch == '+' { "++" } else { "--" };
            let ident_start = skip_ws_forward(expression, byte + operator.len());
            if let Some((ident, ident_end)) = read_identifier_at(expression, ident_start) {
                if let Some((replacement, consumed_end)) = rewrite_js_like_update(
                    ident,
                    expression,
                    ident_start,
                    ident_end,
                    options,
                    &scopes,
                    process_expression_is_arrow_param(
                        &arrow_bindings,
                        ident,
                        ident_start,
                        ident_end,
                    ),
                ) {
                    output.push_str(&replacement);
                    index = chars
                        .iter()
                        .position(|(offset, _)| *offset >= consumed_end)
                        .unwrap_or(chars.len());
                    previous = TokenKind::Other;
                    continue;
                }
            }
        }
        if is_identifier_start(ch) {
            let start = byte;
            index += 1;
            while index < chars.len() && is_identifier_continue(chars[index].1) {
                index += 1;
            }
            let end = chars
                .get(index)
                .map_or(expression.len(), |(offset, _)| *offset);
            let ident = &expression[start..end];
            let arrow_param = process_expression_is_arrow_param(&arrow_bindings, ident, start, end);
            let arrow_local = process_expression_is_arrow_local(&arrow_bindings, ident, start, end);
            let next = next_non_ws(expression, end);
            let prev = previous;
            if !process_expression_update_argument(expression, start, end)
                .is_some_and(|update| update.prefix)
            {
                if let Some((replacement, consumed_end)) = rewrite_js_like_update(
                    ident,
                    expression,
                    start,
                    end,
                    options,
                    &scopes,
                    arrow_param,
                ) {
                    output.push_str(&replacement);
                    index = chars
                        .iter()
                        .position(|(offset, _)| *offset >= consumed_end)
                        .unwrap_or(chars.len());
                    previous = TokenKind::Other;
                    continue;
                }
            }
            if let Some((replacement, consumed_end)) = rewrite_js_like_destructure_identifier(
                ident,
                expression,
                start,
                end,
                options,
                &scopes,
                arrow_param,
            ) {
                output.push_str(&replacement);
                index = chars
                    .iter()
                    .position(|(offset, _)| *offset >= consumed_end)
                    .unwrap_or(chars.len());
                previous = TokenKind::Other;
                continue;
            }
            if let Some((replacement, consumed_end)) = rewrite_js_like_assignment(
                ident,
                expression,
                start,
                end,
                options,
                &scopes,
                arrow_param,
                regular_expression_ranges,
            ) {
                output.push_str(&replacement);
                index = chars
                    .iter()
                    .position(|(offset, _)| *offset >= consumed_end)
                    .unwrap_or(chars.len());
                previous = TokenKind::Other;
                continue;
            }
            if is_keyword(ident) {
                output.push_str(ident);
                match ident {
                    "var" => pending_decl = Some(DeclKind::Var),
                    "let" | "const" => pending_decl = Some(DeclKind::Block),
                    "function" => pending_function_params = true,
                    "for" => for_pending = true,
                    "in" | "of" => pending_decl = None,
                    "catch" => catch_pending = true,
                    _ => {}
                }
                last_keyword = Some(ident.to_string());
                previous = TokenKind::Keyword;
                continue;
            }
            if catch_param_depth.is_some() {
                if next != Some(':') {
                    pending_catch_locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            if pending_decl.is_some()
                && matches!(
                    prev,
                    TokenKind::Keyword | TokenKind::Comma | TokenKind::OpenParen
                )
            {
                if pending_decl == Some(DeclKind::Var) {
                    if let Some(scope) = scopes.first_mut() {
                        scope.locals.push(ident.to_string());
                    }
                } else if for_header_depth.is_some() {
                    pending_for_block_locals.push(ident.to_string());
                } else if let Some(scope) = scopes.last_mut() {
                    scope.locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            let skip_property = matches!(prev, TokenKind::Dot)
                || (next == Some(':') && last_keyword.as_deref() != Some("case"))
                || (pending_function_params
                    && matches!(prev, TokenKind::OpenParen | TokenKind::Comma));
            if skip_property
                || is_global_or_literal(ident)
                || is_generated_asset_import_ident(ident)
                || is_local(&scopes, ident)
                || arrow_param
                || arrow_local
                || pending_for_block_locals.iter().any(|local| local == ident)
            {
                output.push_str(ident);
            } else {
                let content = rewrite_identifier(ident, options);
                output.push_str(&parenthesize_rewritten_identifier_for_new_expression(
                    expression, start, end, &content,
                ));
            }
            previous = TokenKind::Identifier;
            continue;
        }
        output.push(ch);
        match ch {
            '{' => {
                if !pending_for_block_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_for_block_locals),
                    });
                } else if !pending_catch_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_catch_locals),
                    });
                } else {
                    scopes.push(Scope::default());
                }
                previous = TokenKind::Other;
            }
            '}' => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
                previous = TokenKind::Other;
            }
            '(' => {
                paren_depth += 1;
                if for_pending {
                    for_header_depth = Some(paren_depth);
                    for_pending = false;
                }
                if catch_pending {
                    catch_param_depth = Some(paren_depth);
                    catch_pending = false;
                }
                previous = TokenKind::OpenParen;
            }
            ')' => {
                if catch_param_depth == Some(paren_depth) {
                    catch_param_depth = None;
                }
                if for_header_depth == Some(paren_depth) {
                    for_header_depth = None;
                }
                paren_depth = paren_depth.saturating_sub(1);
                pending_function_params = false;
                previous = TokenKind::Other;
            }
            ',' => previous = TokenKind::Comma,
            '.' => previous = TokenKind::Dot,
            ';' => {
                pending_decl = None;
                previous = TokenKind::Other;
            }
            _ if ch.is_whitespace() => {}
            _ => {
                if ch != ':' {
                    last_keyword = None;
                }
                previous = TokenKind::Other;
            }
        }
        index += 1;
    }
}

#[derive(Default)]
struct JsLikeRegExpCollector {
    ranges: Vec<(usize, usize)>,
}

impl<'a> oxc_ast_visit::Visit<'a> for JsLikeRegExpCollector {
    fn visit_reg_exp_literal(&mut self, literal: &oxc_ast::ast::RegExpLiteral<'a>) {
        self.ranges
            .push((literal.span.start as usize, literal.span.end as usize));
    }
}

pub(crate) fn js_like_regular_expression_ranges(
    expression: &str,
    options: &Vue3CompilerOptions,
) -> Vec<(usize, usize)> {
    if !expression.as_bytes().contains(&b'/') {
        return Vec::new();
    }

    let store = JsAstStore::new();
    let source_type = expression_source_type(options);
    if let Ok(parsed) = store.parse_expression(expression, source_type) {
        let mut collector = JsLikeRegExpCollector::default();
        oxc_ast_visit::Visit::visit_expression(&mut collector, &parsed);
        collector.ranges.sort_unstable();
        return collector.ranges;
    }

    const FUNCTION_BODY_PREFIX: &str = "async function __vuec__($event) {\n";
    let wrapped = format!("{FUNCTION_BODY_PREFIX}{expression}\n}}\n");
    let parsed = store.parse_program(&wrapped, source_type);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vec::new();
    }

    let source_start = FUNCTION_BODY_PREFIX.len();
    let source_end = source_start + expression.len();
    let mut collector = JsLikeRegExpCollector::default();
    oxc_ast_visit::Visit::visit_program(&mut collector, &parsed.program);
    let mut ranges = collector
        .ranges
        .into_iter()
        .filter_map(|(start, end)| {
            if start >= source_start && end <= source_end {
                Some((start - source_start, end - source_start))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    ranges
}

fn js_like_regular_expression_ranges_in(
    ranges: &[(usize, usize)],
    source_start: usize,
    source_end: usize,
) -> Vec<(usize, usize)> {
    ranges
        .iter()
        .filter_map(|&(start, end)| {
            if start >= source_start && end <= source_end {
                Some((start - source_start, end - source_start))
            } else {
                None
            }
        })
        .collect()
}

fn js_like_regular_expression_end_at(
    ranges: &[(usize, usize)],
    start: usize,
) -> Option<usize> {
    ranges
        .binary_search_by_key(&start, |&(range_start, _)| range_start)
        .ok()
        .map(|index| ranges[index].1)
}

pub(crate) fn rewrite_template_literal_into(
    expression: &str,
    chars: &[(usize, char)],
    mut index: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    output: &mut String,
    regular_expression_ranges: &[(usize, usize)],
) -> usize {
    output.push('`');
    index += 1;
    while index < chars.len() {
        let ch = chars[index].1;
        output.push(ch);
        index += 1;
        if ch == '\\' && index < chars.len() {
            output.push(chars[index].1);
            index += 1;
            continue;
        }
        if ch == '`' {
            break;
        }
        if ch == '$' && index < chars.len() && chars[index].1 == '{' {
            if let Some(close) = find_template_literal_expression_close(
                expression,
                chars,
                index,
                regular_expression_ranges,
            ) {
                output.push('{');
                let inner_start = chars[index].0 + '{'.len_utf8();
                let inner_end = chars[close].0;
                if let Some(inner) = expression.get(inner_start..inner_end) {
                    let locals = scopes
                        .iter()
                        .flat_map(|scope| scope.locals.iter().cloned())
                        .collect::<Vec<_>>();
                    let inner_ranges = js_like_regular_expression_ranges_in(
                        regular_expression_ranges,
                        inner_start,
                        inner_end,
                    );
                    rewrite_js_like_expression_into_with_ranges(
                        inner,
                        options,
                        locals,
                        output,
                        &inner_ranges,
                    );
                }
                output.push('}');
                index = close + 1;
            }
        }
    }
    index
}

pub(crate) fn find_template_literal_expression_close(
    expression: &str,
    chars: &[(usize, char)],
    mut index: usize,
    regular_expression_ranges: &[(usize, usize)],
) -> Option<usize> {
    let mut depth = 0usize;
    while index < chars.len() {
        let byte = chars[index].0;
        let ch = chars[index].1;
        if ch == '\'' || ch == '"' {
            index = skip_quoted_chars(chars, index, ch);
            continue;
        }
        if ch == '`' {
            index = skip_template_literal_chars(
                expression,
                chars,
                index,
                regular_expression_ranges,
            );
            continue;
        }
        if ch == '/' {
            if let Some(end) =
                js_like_regular_expression_end_at(regular_expression_ranges, byte)
            {
                while index < chars.len() && chars[index].0 < end {
                    index += 1;
                }
                continue;
            }
        }
        if ch == '/'
            && expression
                .get(byte..)
                .is_some_and(|tail| tail.starts_with("//"))
        {
            index += 2;
            while index < chars.len() && !matches!(chars[index].1, '\n' | '\r') {
                index += 1;
            }
            continue;
        }
        if ch == '/'
            && expression
                .get(byte..)
                .is_some_and(|tail| tail.starts_with("/*"))
        {
            index += 2;
            while index < chars.len() {
                if chars[index].1 == '*' && index + 1 < chars.len() && chars[index + 1].1 == '/' {
                    index += 2;
                    break;
                }
                index += 1;
            }
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

pub(crate) fn skip_quoted_chars(chars: &[(usize, char)], mut index: usize, quote: char) -> usize {
    index += 1;
    while index < chars.len() {
        let ch = chars[index].1;
        index += 1;
        if ch == '\\' && index < chars.len() {
            index += 1;
            continue;
        }
        if ch == quote {
            break;
        }
    }
    index
}

pub(crate) fn skip_template_literal_chars(
    expression: &str,
    chars: &[(usize, char)],
    mut index: usize,
    regular_expression_ranges: &[(usize, usize)],
) -> usize {
    index += 1;
    while index < chars.len() {
        let ch = chars[index].1;
        index += 1;
        if ch == '\\' && index < chars.len() {
            index += 1;
            continue;
        }
        if ch == '`' {
            break;
        }
        if ch == '$' && index < chars.len() && chars[index].1 == '{' {
            if let Some(close) = find_template_literal_expression_close(
                expression,
                chars,
                index,
                regular_expression_ranges,
            ) {
                index = close + 1;
            }
        }
    }
    index
}

pub(crate) fn rewrite_js_like_assignment(
    ident: &str,
    expression: &str,
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    arrow_local: bool,
    regular_expression_ranges: &[(usize, usize)],
) -> Option<(String, usize)> {
    if !options.inline || is_local(scopes, ident) || arrow_local {
        return None;
    }
    let binding = options.binding_metadata.get(ident).map(String::as_str)?;
    let assignment = process_expression_assignment_rhs(expression, start, end)?;
    let operator_start = skip_ws_forward(expression, end);
    let rhs_start = skip_ws_forward(expression, operator_start + assignment.operator.len());
    let rhs_end = process_expression_assignment_rhs_end_ignoring_ranges(
        expression,
        rhs_start,
        regular_expression_ranges,
    );
    let rhs_source = expression.get(rhs_start..rhs_end)?;
    let rhs = rhs_source.trim();
    let rhs_source_start = rhs_start + rhs_source.len().saturating_sub(rhs_source.trim_start().len());
    let rhs_ranges = js_like_regular_expression_ranges_in(
        regular_expression_ranges,
        rhs_source_start,
        rhs_source_start + rhs.len(),
    );
    let locals = scopes
        .iter()
        .flat_map(|scope| scope.locals.iter().cloned())
        .collect::<Vec<_>>();
    let mut rewritten_rhs = String::new();
    rewrite_js_like_expression_into_with_ranges(
        rhs,
        options,
        locals,
        &mut rewritten_rhs,
        &rhs_ranges,
    );
    let replacement = match binding {
        "setup-ref" | "setup-maybe-ref" => {
            format!(
                "{ident}.value {} {}",
                assignment.operator,
                rewritten_rhs.trim()
            )
        }
        "setup-let" => {
            format!(
                "_isRef({ident}) ? {ident}.value {} {} : {ident} {} {}",
                assignment.operator,
                rewritten_rhs.trim(),
                assignment.operator,
                rewritten_rhs.trim()
            )
        }
        _ => return None,
    };
    Some((replacement, rhs_end))
}

pub(crate) fn rewrite_js_like_update(
    ident: &str,
    expression: &str,
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    arrow_local: bool,
) -> Option<(String, usize)> {
    if !options.inline || is_local(scopes, ident) || arrow_local {
        return None;
    }
    let binding = options.binding_metadata.get(ident).map(String::as_str)?;
    let update = process_expression_update_argument(expression, start, end)?;
    let (_, consumed_end) = process_expression_update_range(expression, start, end, update)?;
    let prefix = if update.prefix { update.operator } else { "" };
    let postfix = if update.prefix { "" } else { update.operator };
    let replacement = match binding {
        "setup-ref" | "setup-maybe-ref" => format!("{prefix}{ident}.value{postfix}"),
        "setup-let" => {
            format!("_isRef({ident}) ? {prefix}{ident}.value{postfix} : {prefix}{ident}{postfix}")
        }
        _ => return None,
    };
    Some((replacement, consumed_end))
}

pub(crate) fn rewrite_js_like_destructure_identifier(
    ident: &str,
    expression: &str,
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scopes: &[Scope],
    arrow_param: bool,
) -> Option<(String, usize)> {
    if !options.inline
        || is_local(scopes, ident)
        || arrow_param
        || !process_expression_is_destructure_assignment(expression, start)
    {
        return None;
    }
    let binding = options.binding_metadata.get(ident).map(String::as_str)?;
    let rewritten = match binding {
        "setup-ref" | "setup-maybe-ref" => format!("{ident}.value"),
        "setup-let" => ident.to_string(),
        _ => return None,
    };
    if process_expression_object_shorthand(expression, start, end) {
        Some((format!("{ident}: {rewritten}"), end))
    } else {
        Some((rewritten, end))
    }
}

pub(crate) fn process_expression_update_range(
    expression: &str,
    start: usize,
    end: usize,
    update: ProcessExpressionUpdate,
) -> Option<(usize, usize)> {
    if update.prefix {
        let operator_start = previous_operator_start(expression, start, update.operator)?;
        Some((operator_start, end))
    } else {
        let operator_start = skip_ws_forward(expression, end);
        expression
            .get(operator_start..)
            .is_some_and(|tail| tail.starts_with(update.operator))
            .then_some((start, operator_start + update.operator.len()))
    }
}

pub(crate) fn previous_operator_start(
    expression: &str,
    start: usize,
    operator: &str,
) -> Option<usize> {
    let head = expression.get(..start)?.trim_end();
    if !head.ends_with(operator) {
        return None;
    }
    Some(head.len().saturating_sub(operator.len()))
}

pub(crate) fn read_identifier_at(source: &str, start: usize) -> Option<(&str, usize)> {
    let mut chars = source.get(start..)?.char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (relative, ch) in chars {
        if !is_identifier_continue(ch) {
            return Some((&source[start..end], end));
        }
        end = start + relative + ch.len_utf8();
    }
    Some((&source[start..end], end))
}
