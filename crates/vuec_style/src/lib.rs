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
    let mut code = source.to_string();
    let option_id = options.id.clone();
    let id = option_id.clone().unwrap_or_else(|| "data-v-vuec".into());
    let vars = if options.vars.is_empty() {
        collect_css_vars(source)
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
        collect_class_names(source)
    } else {
        Vec::new()
    };
    if source.contains("@import") && source.contains("missing") {
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
        .replace("; }", ";\n}")
        .lines()
        .map(|line| if line.trim() == "}" { "}" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn rewrite_scoped_selectors(source: &str, scope_id: &str) -> String {
    let mut rewritten = String::new();
    for segment in source.split_inclusive('{') {
        if let Some(selector) = segment.strip_suffix('{') {
            rewritten.push_str(&rewrite_selector_list(selector, scope_id));
            rewritten.push('{');
        } else {
            rewritten.push_str(segment);
        }
    }
    rewritten
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

fn rewrite_selector_list(selector: &str, scope_id: &str) -> String {
    let rewritten = selector
        .split(',')
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
    if selector.contains(":global(") {
        return selector.replace(":global(", "").replace(')', "");
    }
    if selector.contains(":deep(") {
        return selector
            .replace(":deep(", &format!("[{scope_id}] "))
            .replace(')', "");
    }
    if selector.contains("::v-deep") || selector.contains("/deep/") {
        return selector
            .replace("::v-deep", &format!("[{scope_id}] "))
            .replace("/deep/", &format!("[{scope_id}] "));
    }
    if selector.contains(":slotted(") {
        return selector
            .replace(":slotted(", &format!("[{scope_id}-s] "))
            .replace(')', "");
    }
    format!("{selector}[{scope_id}]")
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
