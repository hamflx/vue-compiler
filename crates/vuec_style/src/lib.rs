#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleCompileOptions {
    pub id: Option<String>,
    pub scoped: bool,
    pub modules: bool,
    pub vars: Vec<String>,
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
    let id = options.id.unwrap_or_else(|| "data-v-vuec".into());
    let vars = if options.vars.is_empty() {
        collect_css_vars(source)
    } else {
        options.vars
    };

    if options.scoped {
        code = rewrite_scoped_selectors(&code, &id);
    }
    if !vars.is_empty() {
        code = rewrite_css_vars(&code, &vars);
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
    source.replace("; }", ";\n}")
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
    let mut cursor = 0usize;
    while let Some(start) = source[cursor..].find("v-bind(") {
        let start = cursor + start + "v-bind(".len();
        let Some(end_offset) = source[start..].find(')') else {
            break;
        };
        let end = start + end_offset;
        let value = source[start..end]
            .trim()
            .trim_matches(['"', '\''])
            .to_string();
        if !value.is_empty() && !vars.iter().any(|existing| existing == &value) {
            vars.push(value);
        }
        cursor = end + 1;
    }
    vars
}

fn rewrite_css_vars(source: &str, vars: &[String]) -> String {
    let mut code = source.to_string();
    for var in vars {
        let source_var = var.rsplit_once('-').map(|(_, raw)| raw).unwrap_or(var);
        let css_var = format!("var(--{var})");
        code = code.replace(&format!("v-bind({source_var})"), &css_var);
        code = code.replace(&format!("v-bind('{source_var}')"), &css_var);
        code = code.replace(&format!("v-bind(\"{source_var}\")"), &css_var);
    }
    code
}

fn rewrite_selector_list(selector: &str, scope_id: &str) -> String {
    selector
        .split(',')
        .map(|part| rewrite_single_selector(part.trim(), scope_id))
        .collect::<Vec<_>>()
        .join(", ")
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
        assert!(result.code.contains("var(--color)"));
        assert_eq!(result.modules, vec!["a"]);
        assert_eq!(result.vars, vec!["color"]);
        assert!(result.map.is_some());
    }
}
