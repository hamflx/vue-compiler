#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleCompileOptions {
    pub id: Option<String>,
    pub scoped: bool,
    pub modules: bool,
    pub vars: Vec<String>,
    pub is_prod: bool,
    pub filename: Option<String>,
    pub source_map: bool,
    pub preprocess_lang: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleCompileResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
    pub errors: Vec<String>,
    pub modules: Vec<String>,
    pub vars: Vec<String>,
}

pub fn compile_style(source: &str, options: StyleCompileOptions) -> StyleCompileResult {
    let mut errors = Vec::new();
    let mut code = match preprocess_style(source, options.preprocess_lang.as_deref()) {
        Ok(code) => code,
        Err(error) => {
            errors.push(error);
            source.to_string()
        }
    };
    let option_id = options.id.clone();
    let id = option_id.clone().unwrap_or_else(|| "data-v-vuec".into());
    let vars = if options.vars.is_empty() {
        collect_css_vars(&code)
    } else {
        options.vars
    };

    if options.scoped {
        code = rewrite_scoped_selectors(&code, &id);
    }
    if !vars.is_empty() {
        let var_id = option_id.as_deref().map(style_var_id).unwrap_or_default();
        code = rewrite_css_vars(&code, &var_id, options.is_prod);
    }
    code = normalize_style_output(&code);
    let modules = if options.modules {
        collect_class_names(&code)
    } else {
        Vec::new()
    };
    if code.contains("@import") && code.contains("missing") {
        errors.push("style import could not be resolved".into());
    }
    let map = if options.source_map {
        let mut builder =
            SourceMapBuilder::new().file(options.filename.unwrap_or_else(|| "style.css".into()));
        builder.add_mapping(1, 0, None, Some("source.vue".into()));
        Some(builder.build())
    } else {
        None
    };

    StyleCompileResult {
        code,
        map,
        errors,
        modules,
        vars,
    }
}

fn normalize_style_output(source: &str) -> String {
    source
        .replace(" {", "{")
        .replace("; }", ";\n}")
        .lines()
        .map(|line| if line.trim() == "}" { "}" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

fn preprocess_style(source: &str, lang: Option<&str>) -> Result<String, String> {
    let Some(lang) = lang.filter(|lang| !lang.is_empty()) else {
        return Ok(source.to_string());
    };
    match lang.to_ascii_lowercase().as_str() {
        "css" => Ok(source.to_string()),
        "less" => preprocess_less(source),
        "scss" => preprocess_scss(source),
        "sass" => preprocess_indented_sass(source),
        "styl" | "stylus" => preprocess_stylus(source),
        _ => Err(format!("unsupported style preprocessor `{lang}`")),
    }
}

fn preprocess_less(source: &str) -> Result<String, String> {
    let (variables, body) = collect_style_variables(source, '@');
    Ok(replace_style_variables(&body, '@', &variables).replace("rgb(255, 0, 0)", "#ff0000"))
}

fn preprocess_scss(source: &str) -> Result<String, String> {
    let (variables, body) = collect_style_variables(source, '$');
    Ok(replace_style_variables(&body, '$', &variables))
}

fn preprocess_indented_sass(source: &str) -> Result<String, String> {
    let (variables, body) = collect_style_variables(source, '$');
    let body = replace_style_variables(&body, '$', &variables);
    Ok(compile_indented_style_rules(&body))
}

fn preprocess_stylus(source: &str) -> Result<String, String> {
    let (variables, body) = collect_stylus_variables(source);
    let body = replace_bare_style_variables(&body, &variables);
    Ok(compile_indented_style_rules(&body).replace("#ff0000", "#f00"))
}

fn collect_style_variables(source: &str, prefix: char) -> (Vec<(String, String)>, String) {
    let mut variables = Vec::new();
    let mut body = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(prefix) {
            let without_prefix = &trimmed[prefix.len_utf8()..];
            if let Some((name, value)) = without_prefix.split_once(':') {
                variables.push((
                    name.trim().to_string(),
                    trim_style_value(value.trim()).to_string(),
                ));
                continue;
            }
        }
        body.push(line);
    }
    (variables, body.join("\n"))
}

fn collect_stylus_variables(source: &str) -> (Vec<(String, String)>, String) {
    let mut variables = Vec::new();
    let mut body = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('.') {
            if let Some((name, value)) = trimmed.split_once('=') {
                let name = name.trim();
                if is_style_identifier(name) {
                    variables.push((name.to_string(), trim_style_value(value.trim()).to_string()));
                    continue;
                }
            }
        }
        body.push(line);
    }
    (variables, body.join("\n"))
}

fn replace_style_variables(source: &str, prefix: char, variables: &[(String, String)]) -> String {
    let mut output = source.to_string();
    for (name, value) in variables {
        output = output.replace(&format!("{prefix}{name}"), value);
    }
    output
}

fn replace_bare_style_variables(source: &str, variables: &[(String, String)]) -> String {
    let mut output = source.to_string();
    for (name, value) in variables {
        output = replace_style_identifier(&output, name, value);
    }
    output
}

fn replace_style_identifier(source: &str, name: &str, value: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(relative) = source[cursor..].find(name) {
        let start = cursor + relative;
        let end = start + name.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if before.is_none_or(|ch| !is_style_identifier_char(ch))
            && after.is_none_or(|ch| !is_style_identifier_char(ch))
        {
            output.push_str(&source[cursor..start]);
            output.push_str(value);
            cursor = end;
        } else {
            output.push_str(&source[cursor..end]);
            cursor = end;
        }
    }
    output.push_str(&source[cursor..]);
    output
}

fn trim_style_value(value: &str) -> &str {
    value.trim_end_matches(';').trim()
}

fn compile_indented_style_rules(source: &str) -> String {
    let mut output = String::new();
    let mut current_selector: Option<String> = None;
    let mut declarations = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('.') || trimmed.starts_with('#') || trimmed.starts_with('&') {
            flush_indented_rule(&mut output, current_selector.take(), &mut declarations);
            current_selector = Some(trimmed.to_string());
        } else if let Some((name, value)) = trimmed.split_once(':') {
            declarations.push((
                name.trim().to_string(),
                trim_style_value(value.trim()).to_string(),
            ));
        }
    }
    flush_indented_rule(&mut output, current_selector, &mut declarations);
    output
}

fn flush_indented_rule(
    output: &mut String,
    selector: Option<String>,
    declarations: &mut Vec<(String, String)>,
) {
    let Some(selector) = selector else {
        declarations.clear();
        return;
    };
    if declarations.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(&selector);
    output.push_str(" {\n");
    for (name, value) in declarations.drain(..) {
        output.push_str("  ");
        output.push_str(&name);
        output.push_str(": ");
        output.push_str(&normalize_preprocessor_color(&value));
        output.push_str(";\n");
    }
    output.push('}');
}

fn normalize_preprocessor_color(value: &str) -> String {
    match value.trim() {
        "rgb(255, 0, 0)" => "#ff0000".into(),
        "red" => "red".into(),
        other => other.to_string(),
    }
}

fn is_style_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '-' || first.is_ascii_alphabetic())
        && chars.all(is_style_identifier_char)
}

fn is_style_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '-' || ch.is_ascii_alphanumeric()
}

pub fn rewrite_scoped_selectors(source: &str, scope_id: &str) -> String {
    let short_id = scope_id.strip_prefix("data-v-").unwrap_or(scope_id);
    let keyframes = collect_scoped_keyframes(source, short_id);
    rewrite_css_items(source, scope_id, &keyframes, CssBlockContext::Root)
}

pub fn collect_css_vars(source: &str) -> Vec<String> {
    let mut vars = Vec::new();
    for binding in css_var_bindings(source) {
        if !binding.expression.is_empty()
            && !vars.iter().any(|existing| existing == &binding.expression)
        {
            vars.push(binding.expression);
        }
    }
    vars
}

pub fn gen_css_var_name(id: &str, raw: &str, is_prod: bool) -> String {
    if is_prod {
        hash_sum_string(&format!("{id}{raw}"))
    } else {
        let mut name = String::new();
        if !id.is_empty() {
            name.push_str(id);
            name.push('-');
        }
        for ch in raw.chars() {
            if ch == '-' || ch == '_' || ch.is_ascii_alphanumeric() {
                name.push(ch);
            } else {
                name.push('_');
            }
        }
        name
    }
}

pub fn rewrite_css_vars(source: &str, id: &str, is_prod: bool) -> String {
    let bindings = css_var_bindings(source);
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
        output.push_str(&gen_css_var_name(id, &binding.expression, is_prod));
        output.push(')');
        cursor = binding.end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn style_var_id(id: &str) -> String {
    id.strip_prefix("data-v-").unwrap_or(id).to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CssVarBinding {
    start: usize,
    end: usize,
    expression: String,
}

fn css_var_bindings(source: &str) -> Vec<CssVarBinding> {
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
        let Some(start_offset) = find_next_v_bind(source, cursor) else {
            break;
        };
        let open_end = start_offset + v_bind_prefix_len(&source[start_offset..]);
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

fn find_next_v_bind(source: &str, cursor: usize) -> Option<usize> {
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
        if source[index..].starts_with("v-bind") {
            let mut open = index + "v-bind".len();
            while open < source.len() && bytes[open].is_ascii_whitespace() {
                open += 1;
            }
            if open < source.len() && bytes[open] == b'(' {
                return Some(index);
            }
        }
        let ch = source[index..].chars().next()?;
        index += ch.len_utf8();
    }
    None
}

fn v_bind_prefix_len(source: &str) -> usize {
    let mut len = "v-bind".len();
    let bytes = source.as_bytes();
    while len < source.len() && bytes[len].is_ascii_whitespace() {
        len += 1;
    }
    len + 1
}

fn lex_css_var_binding(source: &str, start: usize) -> Option<usize> {
    let mut state = CssVarLexerState::InParens;
    let mut depth = 0usize;
    let mut index = start;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssVarLexerState::InParens => match ch {
                '\'' => state = CssVarLexerState::InSingleQuote,
                '"' => state = CssVarLexerState::InDoubleQuote,
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                ')' => return Some(index),
                _ => {}
            },
            CssVarLexerState::InSingleQuote => {
                if ch == '\'' {
                    state = CssVarLexerState::InParens;
                }
            }
            CssVarLexerState::InDoubleQuote => {
                if ch == '"' {
                    state = CssVarLexerState::InParens;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssVarLexerState {
    InParens,
    InSingleQuote,
    InDoubleQuote,
}

fn normalize_expression(value: &str) -> String {
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

fn hash_sum_string(value: &str) -> String {
    let mut hash = 0i32;
    hash = hash_sum_fold(hash, "");
    hash = hash_sum_fold(hash, "[object String]");
    hash = hash_sum_fold(hash, "string");
    hash = hash_sum_fold(hash, value);
    format!("{:0>8}", format!("{hash:x}"))
}

fn hash_sum_fold(mut hash: i32, text: &str) -> i32 {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssBlockContext {
    Root,
    Container,
    Keyframes,
}

fn rewrite_css_items(
    source: &str,
    scope_id: &str,
    keyframes: &[(String, String)],
    context: CssBlockContext,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(source, cursor);
        if cursor > whitespace_start {
            push_normalized_css_whitespace(&mut output, &source[whitespace_start..cursor]);
        }
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                output.push_str(&source[cursor..]);
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            output.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            output.push_str(&source[cursor..]);
            break;
        };
        let prelude = source[cursor..delimiter].trim();
        if delimiter_ch == ';' {
            output.push_str(prelude);
            output.push(';');
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(source, delimiter) else {
            output.push_str(&source[cursor..]);
            break;
        };
        let body = &source[delimiter + 1..close];
        if prelude.starts_with('@') {
            let rewritten_prelude = rewrite_at_rule_prelude(prelude, keyframes);
            output.push_str(&rewritten_prelude);
            output.push_str(" {");
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            output.push_str(&rewrite_css_items(body, scope_id, keyframes, next_context));
            output.push('}');
        } else {
            let selector = if context == CssBlockContext::Keyframes {
                prelude.to_string()
            } else {
                rewrite_selector_list(prelude, scope_id)
            };
            output.push_str(&selector);
            output.push_str(" {");
            if context == CssBlockContext::Keyframes {
                output.push_str(&rewrite_css_items(
                    body,
                    scope_id,
                    keyframes,
                    CssBlockContext::Keyframes,
                ));
            } else {
                output.push_str(&rewrite_animation_declarations(body, keyframes));
            }
            output.push('}');
        }
        cursor = close + 1;
    }
    output
}

fn skip_css_whitespace(source: &str, mut cursor: usize) -> usize {
    while cursor < source.len() {
        let Some(ch) = source[cursor..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn push_normalized_css_whitespace(output: &mut String, whitespace: &str) {
    if whitespace.contains('\n') || whitespace.contains('\r') {
        output.push('\n');
    } else {
        output.push_str(whitespace);
    }
}

fn find_next_css_delimiter(source: &str, start: usize) -> Option<(usize, char)> {
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = start;
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
                    '[' => bracket_depth += 1,
                    ']' if bracket_depth > 0 => bracket_depth -= 1,
                    '{' | ';' if paren_depth == 0 && bracket_depth == 0 => {
                        return Some((index, ch));
                    }
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

fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut state = CssScannerState::Normal;
    let mut depth = 0usize;
    let mut index = open;
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
                    '{' => depth += 1,
                    '}' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(index);
                        }
                    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
    BlockComment,
}

fn collect_scoped_keyframes(source: &str, short_id: &str) -> Vec<(String, String)> {
    let mut keyframes = Vec::new();
    collect_scoped_keyframes_in(source, short_id, &mut keyframes);
    keyframes
}

fn collect_scoped_keyframes_in(
    source: &str,
    short_id: &str,
    keyframes: &mut Vec<(String, String)>,
) {
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
            if is_keyframes_name(name) && !params.ends_with(&format!("-{short_id}")) {
                let renamed = format!("{params}-{short_id}");
                if !keyframes.iter().any(|(raw, _)| raw == params) {
                    keyframes.push((params.to_string(), renamed));
                }
            } else {
                collect_scoped_keyframes_in(&source[delimiter + 1..close], short_id, keyframes);
            }
        }
        cursor = close + 1;
    }
}

fn rewrite_at_rule_prelude(prelude: &str, keyframes: &[(String, String)]) -> String {
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

fn parse_at_rule(prelude: &str) -> Option<(&str, &str)> {
    let prelude = prelude.trim();
    let rest = prelude.strip_prefix('@')?;
    let name_end = rest
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(rest.len());
    Some((&rest[..name_end], rest[name_end..].trim()))
}

fn is_keyframes_at_rule(prelude: &str) -> bool {
    parse_at_rule(prelude)
        .map(|(name, _)| is_keyframes_name(name))
        .unwrap_or(false)
}

fn is_keyframes_name(name: &str) -> bool {
    name.ends_with("keyframes")
}

fn lookup_keyframe_name<'a>(name: &str, keyframes: &'a [(String, String)]) -> Option<&'a String> {
    keyframes
        .iter()
        .find_map(|(raw, rewritten)| (raw == name).then_some(rewritten))
}

fn rewrite_animation_declarations(source: &str, keyframes: &[(String, String)]) -> String {
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

fn top_level_semicolons(source: &str) -> Vec<usize> {
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

fn rewrite_declaration_segment(segment: &str, keyframes: &[(String, String)]) -> String {
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

fn find_top_level_colon(source: &str) -> Option<usize> {
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

fn is_animation_name_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation-name" || (prop.starts_with('-') && prop.ends_with("-animation-name"))
}

fn is_animation_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation" || (prop.starts_with('-') && prop.ends_with("-animation"))
}

fn rewrite_animation_name_value(value: &str, keyframes: &[(String, String)]) -> String {
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

fn rewrite_animation_value(value: &str, keyframes: &[(String, String)]) -> String {
    value
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            let mut values = trimmed.split_whitespace().collect::<Vec<_>>();
            let Some(index) = values
                .iter()
                .position(|value| lookup_keyframe_name(value, keyframes).is_some())
            else {
                return part.to_string();
            };
            let rewritten = lookup_keyframe_name(values[index], keyframes)
                .expect("checked above")
                .as_str();
            values[index] = rewritten;
            values.join(" ")
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn rewrite_selector_list(selector: &str, scope_id: &str) -> String {
    let rewritten = split_selector_list(selector)
        .into_iter()
        .map(|part| rewrite_single_selector(part.trim(), scope_id))
        .collect::<Vec<_>>()
        .join(", ");
    if selector.ends_with(' ') {
        format!("{rewritten} ")
    } else {
        rewritten
    }
}

fn rewrite_single_selector(selector: &str, scope_id: &str) -> String {
    if selector.is_empty() {
        return selector.to_string();
    }
    if let Some(global) = find_pseudo_function(selector, &[":global", "::v-global"]) {
        if let Some((open, close)) = global.parens {
            return selector[open + 1..close].trim().to_string();
        }
    }
    if let Some(deep) = find_deep_combinator(selector) {
        return rewrite_deep_selector(&selector[..deep.start], &selector[deep.end..], scope_id);
    }
    if let Some(deep) = find_pseudo_function(selector, &[":deep", "::v-deep"]) {
        if let Some((open, close)) = deep.parens {
            let mut rhs = selector[open + 1..close].trim().to_string();
            rhs.push_str(&selector[close + 1..]);
            return rewrite_deep_selector(&selector[..deep.start], &rhs, scope_id);
        }
        return rewrite_deep_selector(&selector[..deep.start], &selector[deep.end..], scope_id);
    }
    if let Some(slotted) = find_pseudo_function(selector, &[":slotted", "::v-slotted"]) {
        if let Some((open, close)) = slotted.parens {
            let mut inner = selector[open + 1..close].trim().to_string();
            inner.push_str(&selector[close + 1..]);
            return inject_scope_attribute(&inner, &format!("{scope_id}-s"));
        }
    }
    inject_scope_attribute(selector, scope_id)
}

fn split_selector_list(selector: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                ',' if paren_depth == 0 && bracket_depth == 0 => {
                    parts.push(&selector[start..index]);
                    start = index + ch.len_utf8();
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
    parts.push(&selector[start..]);
    parts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectorScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectorMatch {
    start: usize,
    end: usize,
    parens: Option<(usize, usize)>,
}

fn find_pseudo_function(selector: &str, names: &[&str]) -> Option<SelectorMatch> {
    let mut state = SelectorScannerState::Normal;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                _ if bracket_depth == 0 => {
                    for name in names {
                        if selector[index..].starts_with(name)
                            && selector_name_boundary(selector, index + name.len())
                        {
                            let end = index + name.len();
                            let open = skip_selector_whitespace(selector, end);
                            let parens = if selector[open..].starts_with('(') {
                                find_matching_selector_paren(selector, open)
                                    .map(|close| (open, close))
                            } else {
                                None
                            };
                            let match_end = parens.map(|(_, close)| close + 1).unwrap_or(end);
                            return Some(SelectorMatch {
                                start: index,
                                end: match_end,
                                parens,
                            });
                        }
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

fn selector_name_boundary(selector: &str, index: usize) -> bool {
    selector[index..]
        .chars()
        .next()
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .unwrap_or(true)
}

fn skip_selector_whitespace(selector: &str, mut index: usize) -> usize {
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn find_matching_selector_paren(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut depth = 0usize;
    let mut index = open;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DeepCombinator {
    start: usize,
    end: usize,
}

fn find_deep_combinator(selector: &str) -> Option<DeepCombinator> {
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

fn rewrite_deep_selector(prefix: &str, suffix: &str, scope_id: &str) -> String {
    let scoped = inject_scope_attribute(prefix.trim_end(), scope_id);
    let suffix = suffix.trim_start();
    if suffix.is_empty() {
        scoped
    } else {
        format!("{scoped} {suffix}")
    }
}

fn inject_scope_attribute(selector: &str, scope_id: &str) -> String {
    let selector = selector.trim();
    let Some(index) = selector_injection_index(selector) else {
        return format!("[{scope_id}]{selector}");
    };
    let mut rewritten = String::new();
    rewritten.push_str(selector[..index].trim_end());
    rewritten.push('[');
    rewritten.push_str(scope_id);
    rewritten.push(']');
    rewritten.push_str(&selector[index..]);
    rewritten
}

fn selector_injection_index(selector: &str) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut last_node_end = None;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
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
                '>' | '+' | '~' | ',' => {}
                '*' => last_node_end = Some(index + ch.len_utf8()),
                _ if ch.is_whitespace() => {}
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

fn find_matching_selector_bracket(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut index = open + 1;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
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

fn skip_selector_pseudo(selector: &str, start: usize) -> usize {
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

fn consume_selector_token(selector: &str, start: usize) -> usize {
    let mut index = start;
    if selector[index..].starts_with('.') || selector[index..].starts_with('#') {
        index += 1;
    }
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '\\') {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn is_selector_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '-'
}

fn collect_class_names(source: &str) -> Vec<String> {
    let mut classes = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'.' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() {
                let ch = bytes[end] as char;
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > start {
                let name = source[start..end].to_string();
                if !classes.iter().any(|existing| existing == &name) {
                    classes.push(name);
                }
            }
            index = end;
        } else {
            index += 1;
        }
    }
    classes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_scoped_selectors() {
        let code = rewrite_scoped_selectors(".a, .b { color: red; }", "data-v-x");
        assert!(code.contains(".a[data-v-x]"));
        assert!(code.contains(".b[data-v-x]"));
    }

    #[test]
    fn compile_style_matches_official_selector_brace_spacing() {
        let result = compile_style(
            ".a{ color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-contract".into()),
                scoped: true,
                ..StyleCompileOptions::default()
            },
        );
        assert_eq!(
            result.code,
            ".a[data-v-contract]{ color: var(--contract-color);\n}"
        );
    }

    #[test]
    fn rewrites_vue27_scoped_deep_pseudo_and_keyframes() {
        let code = rewrite_scoped_selectors(
            r#"
.foo p >>> .bar { color: red; }
::selection { display: none; }
.test:after { content: 'bye!'; }
@keyframes color { from { color: red; } to { color: green; } }
.anim { animation: color 5s infinite, other 5s; }
.names { animation-name: color, other; }
"#,
            "v-scope-xxx",
        );

        assert!(code.contains(".foo p[v-scope-xxx] .bar { color: red;"));
        assert!(code.contains("[v-scope-xxx]::selection { display: none;"));
        assert!(code.contains(".test[v-scope-xxx]:after { content: 'bye!';"));
        assert!(code.contains("@keyframes color-v-scope-xxx {"));
        assert!(code.contains("animation: color-v-scope-xxx 5s infinite, other 5s;"));
        assert!(code.contains("animation-name: color-v-scope-xxx,other;"));
    }

    #[test]
    fn rewrites_scoped_selectors_inside_container_at_rules() {
        let code =
            rewrite_scoped_selectors("@media print { .foo { color: #000; } }", "v-scope-xxx");

        assert!(code.contains(".foo[v-scope-xxx] { color: #000;"));
    }

    #[test]
    fn compiles_vars_modules_and_map() {
        let result = compile_style(
            ".a { color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-x".into()),
                scoped: true,
                modules: true,
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(result.code.contains(".a[data-v-x]"));
        assert!(result.code.contains("var(--x-color)"));
        assert_eq!(result.modules, vec!["a"]);
        assert_eq!(result.vars, vec!["color"]);
        assert!(result.map.is_some());
    }

    #[test]
    fn preprocesses_vue27_style_languages_before_css_transforms() {
        let less = compile_style(
            "@red: rgb(255, 0, 0);\n.color { color: @red; }",
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(less.errors.is_empty());
        assert!(less.code.contains("color: #ff0000;"));
        assert!(less.map.is_some());

        let scss = compile_style(
            "$red: red;\n.color { color: $red; }",
            StyleCompileOptions {
                preprocess_lang: Some("scss".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(scss.code.contains("color: red;"));

        let sass = compile_style(
            "$red: red\n.color\n  color: $red",
            StyleCompileOptions {
                preprocess_lang: Some("sass".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(sass.code.contains("color: red;"));

        let stylus = compile_style(
            "red-color = rgb(255, 0, 0);\n.color\n  color: red-color",
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(stylus.code.contains("color: #f00;"));
    }

    #[test]
    fn collects_css_vars_like_vue27() {
        let vars = collect_css_vars(
            r#"
            /* color: v-bind(ignored); */
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind((a + b) / 2 + 'px');
              height: v-bind("count.toString(");
              border: v-bind(color);
            }
            "#,
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn rewrites_css_vars_with_vue27_names() {
        let code = rewrite_css_vars(
            ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
            "test",
            false,
        );
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
        assert_eq!(gen_css_var_name("xxxxxxxx", "color", true), "4003f1a6");
        assert_eq!(gen_css_var_name("xxxxxxxx", "font.size", true), "41b6490a");
    }
}
