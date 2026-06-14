use crate::*;

pub(crate) fn preprocess_vue27_template(
    source: &str,
    options: Vue27TemplatePreprocessOptions,
) -> Vue27TemplatePreprocessResult {
    let Some(lang) = options.lang.as_deref().filter(|lang| !lang.is_empty()) else {
        return Vue27TemplatePreprocessResult {
            source: source.to_string(),
            errors: Vec::new(),
            tips: Vec::new(),
        };
    };
    match lang.to_ascii_lowercase().as_str() {
        "html" => Vue27TemplatePreprocessResult {
            source: source.to_string(),
            errors: Vec::new(),
            tips: Vec::new(),
        },
        "pug" | "jade" => match compile_vue27_pug_template(source) {
            Ok(source) => Vue27TemplatePreprocessResult {
                source,
                errors: Vec::new(),
                tips: Vec::new(),
            },
            Err(error) => Vue27TemplatePreprocessResult {
                source: source.to_string(),
                errors: vec![error],
                tips: Vec::new(),
            },
        },
        _ => {
            let filename = options.filename.unwrap_or_else(|| "anonymous.vue".into());
            Vue27TemplatePreprocessResult {
                source: source.to_string(),
                tips: vec![format!(
                    "Component {filename} uses lang {lang} for template. Please install the language preprocessor."
                )],
                errors: vec![format!(
                    "Component {filename} uses lang {lang} for template, however it is not installed."
                )],
            }
        }
    }
}

pub(crate) fn preprocess_vue3_template(
    source: &str,
    options: Vue3TemplatePreprocessOptions,
) -> Vue3TemplatePreprocessResult {
    let Some(lang) = options.lang.as_deref().filter(|lang| !lang.is_empty()) else {
        return Vue3TemplatePreprocessResult {
            source: source.to_string(),
            errors: Vec::new(),
            tips: Vec::new(),
        };
    };
    let filename = options.filename.unwrap_or_else(|| "anonymous.vue".into());
    match lang.to_ascii_lowercase().as_str() {
        "html" => Vue3TemplatePreprocessResult {
            source: source.to_string(),
            errors: Vec::new(),
            tips: Vec::new(),
        },
        "pug" | "jade" => match compile_vue3_pug_template(source, &filename) {
            Ok(source) => Vue3TemplatePreprocessResult {
                source,
                errors: Vec::new(),
                tips: Vec::new(),
            },
            Err(error) => Vue3TemplatePreprocessResult {
                source: source.to_string(),
                errors: vec![error],
                tips: Vec::new(),
            },
        },
        _ => Vue3TemplatePreprocessResult {
            source: source.to_string(),
            tips: vec![format!(
                "Component {filename} uses lang {lang} for template. Please install the language preprocessor."
            )],
            errors: vec![format!(
                "Component {filename} uses lang {lang} for template, however it is not installed."
            )],
        },
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue27PugNode {
    pub(crate) tag: String,
    pub(crate) attrs: Vec<Vue27PugAttr>,
    pub(crate) text: Option<String>,
    pub(crate) children: Vec<Vue27PugNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27PugAttr {
    pub(crate) name: String,
    pub(crate) value: Option<String>,
}

pub(crate) fn compile_vue27_pug_template(source: &str) -> Result<String, String> {
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let trimmed_end = line.trim_end();
        let content = trimmed_end.trim_start();
        if content.is_empty() || content.starts_with("//") {
            continue;
        }
        let indent = vue27_pug_indent(trimmed_end);
        let node = parse_vue27_pug_line(content).map_err(|error| {
            format!(
                "Pug template parse error on line {}: {error}",
                line_index + 1
            )
        })?;
        while stack
            .last()
            .is_some_and(|(parent_indent, _)| *parent_indent >= indent)
        {
            stack.pop();
        }
        let parent_path = stack
            .last()
            .map(|(_, path)| path.clone())
            .unwrap_or_default();
        let children = vue27_pug_children_at_path(&mut roots, &parent_path);
        let index = children.len();
        children.push(node);
        let mut path = parent_path;
        path.push(index);
        stack.push((indent, path));
    }
    Ok(render_vue27_pug_nodes(&roots))
}

pub(crate) fn compile_vue3_pug_template(source: &str, filename: &str) -> Result<String, String> {
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let trimmed_end = line.trim_end();
        let content = trimmed_end.trim_start();
        if content.is_empty() || content.starts_with("//") {
            continue;
        }
        let indent = vue27_pug_indent(trimmed_end);
        let node = parse_vue27_pug_line(content).map_err(|error| {
            let line_number = vue3_pug_public_line_number(source, line_index + 1);
            format!(
                "Error: {filename}:{line_number}:1\n{}",
                vue3_pug_public_error_message(&error)
            )
        })?;
        while stack
            .last()
            .is_some_and(|(parent_indent, _)| *parent_indent >= indent)
        {
            stack.pop();
        }
        let parent_path = stack
            .last()
            .map(|(_, path)| path.clone())
            .unwrap_or_default();
        let children = vue27_pug_children_at_path(&mut roots, &parent_path);
        let index = children.len();
        children.push(node);
        let mut path = parent_path;
        path.push(index);
        stack.push((indent, path));
    }
    Ok(render_vue27_pug_nodes(&roots))
}

pub(crate) fn vue3_pug_public_line_number(source: &str, local_line_number: usize) -> usize {
    if source.starts_with('\n') {
        local_line_number + 1
    } else {
        local_line_number
    }
}

pub(crate) fn vue3_pug_public_error_message(error: &str) -> String {
    if error == "missing closing attribute paren" {
        "The end of the string reached with no closing bracket ) found.".into()
    } else {
        error.to_string()
    }
}

pub(crate) fn vue27_pug_indent(line: &str) -> usize {
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .map(|ch| if ch == '\t' { 2 } else { 1 })
        .sum()
}

pub(crate) fn vue27_pug_children_at_path<'a>(
    roots: &'a mut Vec<Vue27PugNode>,
    path: &[usize],
) -> &'a mut Vec<Vue27PugNode> {
    let mut current = roots;
    for &index in path {
        current = &mut current[index].children;
    }
    current
}

pub(crate) fn parse_vue27_pug_line(source: &str) -> Result<Vue27PugNode, String> {
    if let Some(text) = source.strip_prefix('|') {
        return Ok(Vue27PugNode {
            tag: "span".into(),
            text: Some(text.trim_start().to_string()),
            ..Vue27PugNode::default()
        });
    }
    let mut rest = source;
    let tag = if rest.starts_with('.') || rest.starts_with('#') {
        "div".to_string()
    } else {
        let (name, tail) = take_vue27_pug_name(rest);
        if name.is_empty() {
            return Err("expected tag name".into());
        }
        rest = tail;
        name.to_string()
    };
    let mut attrs = Vec::new();
    let mut shorthand_classes = Vec::new();
    let mut shorthand_id = None;
    loop {
        if let Some(tail) = rest.strip_prefix('.') {
            let (name, next) = take_vue27_pug_name(tail);
            if name.is_empty() {
                return Err("expected class name".into());
            }
            shorthand_classes.push(name.to_string());
            rest = next;
        } else if let Some(tail) = rest.strip_prefix('#') {
            let (name, next) = take_vue27_pug_name(tail);
            if name.is_empty() {
                return Err("expected id".into());
            }
            shorthand_id = Some(name.to_string());
            rest = next;
        } else {
            break;
        }
    }
    if rest.starts_with('(') {
        let (raw_attrs, tail) = take_vue27_pug_attrs(rest)?;
        attrs.extend(parse_vue27_pug_attrs(raw_attrs));
        rest = tail;
    }
    if let Some(id) = shorthand_id {
        if !attrs.iter().any(|attr| attr.name == "id") {
            attrs.push(Vue27PugAttr {
                name: "id".into(),
                value: Some(id),
            });
        }
    }
    if !shorthand_classes.is_empty() {
        let shorthand = shorthand_classes.join(" ");
        if let Some(class_attr) = attrs.iter_mut().find(|attr| attr.name == "class") {
            let existing = class_attr.value.get_or_insert_with(String::new);
            if existing.is_empty() {
                existing.push_str(&shorthand);
            } else {
                existing.push(' ');
                existing.push_str(&shorthand);
            }
        } else {
            attrs.push(Vue27PugAttr {
                name: "class".into(),
                value: Some(shorthand),
            });
        }
    }
    let text = rest.trim_start();
    Ok(Vue27PugNode {
        tag,
        attrs,
        text: (!text.is_empty()).then(|| text.to_string()),
        children: Vec::new(),
    })
}

pub(crate) fn take_vue27_pug_name(source: &str) -> (&str, &str) {
    let end = source
        .char_indices()
        .find_map(|(index, ch)| {
            (!(ch == '-' || ch == '_' || ch == ':' || ch.is_ascii_alphanumeric())).then_some(index)
        })
        .unwrap_or(source.len());
    (&source[..end], &source[end..])
}

pub(crate) fn take_vue27_pug_attrs(source: &str) -> Result<(&str, &str), String> {
    let mut depth = 0usize;
    let mut quote = None;
    for (index, ch) in source.char_indices() {
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok((&source[1..index], &source[index + ch.len_utf8()..]));
                }
            }
            _ => {}
        }
    }
    Err("missing closing attribute paren".into())
}

pub(crate) fn parse_vue27_pug_attrs(source: &str) -> Vec<Vue27PugAttr> {
    split_vue27_pug_attrs(source)
        .into_iter()
        .filter_map(|raw| {
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            let Some((name, value)) = raw.split_once('=') else {
                return Some(Vue27PugAttr {
                    name: raw.to_string(),
                    value: None,
                });
            };
            Some(Vue27PugAttr {
                name: name.trim().to_string(),
                value: Some(trim_vue27_pug_attr_value(value.trim()).to_string()),
            })
        })
        .collect()
}

pub(crate) fn split_vue27_pug_attrs(source: &str) -> Vec<&str> {
    let mut attrs = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    for (index, ch) in source.char_indices() {
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            ',' => {
                attrs.push(&source[start..index]);
                start = index + ch.len_utf8();
            }
            ch if ch.is_whitespace() => {
                let raw = &source[start..index];
                if raw.contains('=') {
                    attrs.push(raw);
                    start = index + ch.len_utf8();
                }
            }
            _ => {}
        }
    }
    if start <= source.len() {
        attrs.push(&source[start..]);
    }
    attrs
}

pub(crate) fn trim_vue27_pug_attr_value(source: &str) -> &str {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(source)
}

pub(crate) fn render_vue27_pug_nodes(nodes: &[Vue27PugNode]) -> String {
    nodes.iter().map(render_vue27_pug_node).collect()
}

pub(crate) fn render_vue27_pug_node(node: &Vue27PugNode) -> String {
    let mut output = String::new();
    output.push('<');
    output.push_str(&node.tag);
    for attr in &node.attrs {
        output.push(' ');
        output.push_str(&attr.name);
        if let Some(value) = attr.value.as_ref() {
            output.push_str("=\"");
            output.push_str(&escape_vue27_pug_attr(value));
            output.push('"');
        }
    }
    output.push('>');
    if let Some(text) = node.text.as_ref() {
        output.push_str(&escape_vue27_pug_text(text));
    }
    output.push_str(&render_vue27_pug_nodes(&node.children));
    output.push_str("</");
    output.push_str(&node.tag);
    output.push('>');
    output
}

pub(crate) fn escape_vue27_pug_text(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn escape_vue27_pug_attr(source: &str) -> String {
    source.replace('&', "&amp;").replace('"', "&quot;")
}
