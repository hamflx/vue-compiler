use crate::*;

/// Collects unique CSS variable expressions from `v-bind(...)` calls.
pub fn collect_css_vars(source: &str) -> Vec<String> {
    collect_css_vars_with_options(source, CssVarCollectOptions::default())
}

/// Options for collecting CSS variable expressions from `v-bind(...)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CssVarCollectOptions {
    /// Whether Less/Sass/Stylus-style `// ...` comments are skipped.
    pub ignore_line_comments: bool,
}

/// Collects unique CSS variable expressions from `v-bind(...)` calls.
pub fn collect_css_vars_with_options(source: &str, options: CssVarCollectOptions) -> Vec<String> {
    let mut vars = Vec::new();
    for binding in css_var_bindings(
        source,
        options.ignore_line_comments,
        CssVarScanMode::Collect,
    ) {
        if !binding.expression.is_empty()
            && !vars.iter().any(|existing| existing == &binding.expression)
        {
            vars.push(binding.expression);
        }
    }
    vars
}

/// Generates the CSS custom property name for a Vue style variable binding.
pub fn gen_css_var_name(id: &str, raw: &str, is_prod: bool) -> String {
    gen_css_var_name_with_style(id, raw, is_prod, CssVarNameStyle::Vue27Legacy)
}

/// Generates the CSS custom property name for a Vue style variable binding.
pub fn gen_css_var_name_with_style(
    id: &str,
    raw: &str,
    is_prod: bool,
    style: CssVarNameStyle,
) -> String {
    if is_prod {
        let hash = hash_sum_string(&format!("{id}{raw}"));
        return if matches!(style, CssVarNameStyle::Vue3Escaped)
            && hash.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        {
            format!("v{hash}")
        } else {
            hash
        };
    }
    let mut name = String::new();
    if !id.is_empty() {
        name.push_str(id);
        name.push('-');
    }
    match style {
        CssVarNameStyle::Vue3Escaped => {
            name.push_str(&escape_vue3_css_var_name(raw));
        }
        CssVarNameStyle::Vue27Legacy => {
            name.push_str(&legacy_vue27_css_var_name(raw));
        }
    }
    name
}

/// Rewrites `v-bind(...)` CSS expressions to `var(--...)` custom properties.
pub fn rewrite_css_vars(source: &str, id: &str, is_prod: bool) -> String {
    rewrite_css_vars_with_options(
        source,
        id,
        CssVarRewriteOptions {
            is_prod,
            name_style: CssVarNameStyle::Vue27Legacy,
            ignore_line_comments: false,
        },
    )
}

/// Options for rewriting CSS variable bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssVarRewriteOptions {
    /// Whether production CSS variable names should use hashed names.
    pub is_prod: bool,
    /// CSS variable custom-property naming behavior.
    pub name_style: CssVarNameStyle,
    /// Whether Less/Sass/Stylus-style `// ...` comments are skipped.
    pub ignore_line_comments: bool,
}

/// Rewrites `v-bind(...)` CSS expressions to `var(--...)` custom properties.
pub fn rewrite_css_vars_with_options(
    source: &str,
    id: &str,
    options: CssVarRewriteOptions,
) -> String {
    let bindings = css_var_bindings(
        source,
        options.ignore_line_comments,
        CssVarScanMode::Rewrite,
    );
    if bindings.is_empty() {
        return source.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    for binding in bindings {
        if binding.start < cursor {
            continue;
        }
        output.push_str(&source[cursor..binding.start]);
        output.push_str("var(--");
        output.push_str(&gen_css_var_name_with_style(
            id,
            &binding.expression,
            options.is_prod,
            options.name_style,
        ));
        output.push(')');
        cursor = binding.end;
    }
    output.push_str(&source[cursor..]);
    output
}

pub(crate) fn escape_vue3_css_var_name(raw: &str) -> String {
    let mut escaped = String::new();
    for ch in raw.chars() {
        if is_vue3_css_var_escape_symbol(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

pub(crate) fn is_vue3_css_var_escape_symbol(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '!'
            | '"'
            | '#'
            | '$'
            | '%'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | '.'
            | '/'
            | ':'
            | ';'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '['
            | '\\'
            | ']'
            | '^'
            | '`'
            | '{'
            | '|'
            | '}'
            | '~'
    )
}

pub(crate) fn legacy_vue27_css_var_name(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch == '-' || ch == '_' || ch.is_ascii_alphanumeric() {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn style_var_id(id: &str) -> String {
    id.strip_prefix("data-v-").unwrap_or(id).to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssVarBinding {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) expression: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssVarScanMode {
    Collect,
    Rewrite,
}

pub(crate) fn css_var_bindings(
    source: &str,
    ignore_line_comments: bool,
    mode: CssVarScanMode,
) -> Vec<CssVarBinding> {
    let mut bindings = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        if ignore_line_comments && source[cursor..].starts_with("//") {
            cursor = skip_css_line_comment(source, cursor);
            continue;
        }
        let Some((start_offset, open_end)) =
            find_next_v_bind(source, cursor, ignore_line_comments, mode)
        else {
            break;
        };
        let Some(end) = lex_css_var_binding(source, open_end) else {
            cursor = open_end;
            continue;
        };
        bindings.push(CssVarBinding {
            start: start_offset,
            end: end + 1,
            expression: normalize_expression(&source[open_end..end]),
        });
        cursor = end + 1;
    }
    bindings
}

pub(crate) fn find_next_v_bind(
    source: &str,
    cursor: usize,
    ignore_line_comments: bool,
    mode: CssVarScanMode,
) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut index = cursor;
    while index + "v-bind".len() <= source.len() {
        if source[index..].starts_with("/*") {
            if let Some(end_offset) = source[index + 2..].find("*/") {
                index += 2 + end_offset + 2;
                continue;
            }
            return None;
        }
        if ignore_line_comments && source[index..].starts_with("//") {
            index = skip_css_line_comment(source, index);
            continue;
        }
        if source[index..].starts_with("v-bind") {
            let mut open = index + "v-bind".len();
            let mut saw_comment = false;
            let mut saw_whitespace = false;
            while open < source.len() {
                if bytes[open].is_ascii_whitespace() {
                    saw_whitespace = true;
                    open += 1;
                    continue;
                }
                if source[open..].starts_with("/*") {
                    let end_offset = source[open + 2..].find("*/")?;
                    saw_comment = true;
                    open += 2 + end_offset + 2;
                    continue;
                }
                if ignore_line_comments && source[open..].starts_with("//") {
                    saw_comment = true;
                    open = skip_css_line_comment(source, open);
                    continue;
                }
                break;
            }
            if open < source.len()
                && bytes[open] == b'('
                && (mode == CssVarScanMode::Collect || !saw_comment || saw_whitespace)
            {
                return Some((index, open + 1));
            }
        }
        let ch = source[index..].chars().next()?;
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn skip_css_line_comment(source: &str, start: usize) -> usize {
    source[start..]
        .find(['\n', '\r'])
        .map(|offset| start + offset)
        .unwrap_or(source.len())
}

pub(crate) fn lex_css_var_binding(source: &str, start: usize) -> Option<usize> {
    let mut state = CssVarLexerState::Parens;
    let mut depth = 0usize;
    let mut index = start;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssVarLexerState::Parens => match ch {
                '\'' => state = CssVarLexerState::SingleQuote,
                '"' => state = CssVarLexerState::DoubleQuote,
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                ')' => return Some(index),
                _ => {}
            },
            CssVarLexerState::SingleQuote => {
                if ch == '\'' {
                    state = CssVarLexerState::Parens;
                }
            }
            CssVarLexerState::DoubleQuote => {
                if ch == '"' {
                    state = CssVarLexerState::Parens;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssVarLexerState {
    Parens,
    SingleQuote,
    DoubleQuote,
}

pub(crate) fn normalize_expression(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn hash_sum_string(value: &str) -> String {
    let mut hash = 0i32;
    hash = hash_sum_fold(hash, "");
    hash = hash_sum_fold(hash, "[object String]");
    hash = hash_sum_fold(hash, "string");
    hash = hash_sum_fold(hash, value);
    format!("{:0>8}", format!("{hash:x}"))
}

pub(crate) fn hash_sum_fold(mut hash: i32, text: &str) -> i32 {
    if text.is_empty() {
        return hash;
    }
    for code in text.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(code as i32);
    }
    if hash < 0 {
        hash.wrapping_mul(-2)
    } else {
        hash
    }
}
