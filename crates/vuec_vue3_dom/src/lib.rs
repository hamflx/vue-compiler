#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    NodeId, TemplateAttribute, Vue3Ast, Vue3AstKind, Vue3Element, Vue3ElementType, Vue3Prop,
};
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_pass::TransformContext;
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomCompilerOptions {
    pub core: Vue3CompilerOptions,
    pub is_custom_element: Vec<String>,
    pub transform_asset_urls: bool,
    pub decode_entities: bool,
}

impl Default for DomCompilerOptions {
    fn default() -> Self {
        let mut core = Vue3CompilerOptions::default();
        core.built_in_components = vec![
            "Transition".into(),
            "transition".into(),
            "TransitionGroup".into(),
            "transition-group".into(),
        ];
        Self {
            core,
            is_custom_element: Vec::new(),
            transform_asset_urls: true,
            decode_entities: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomDirective {
    pub name: String,
    pub argument: Option<String>,
    pub modifiers: Vec<String>,
    pub expression: Option<String>,
}

pub fn parse(source: TemplateSource, options: &DomCompilerOptions) -> Vue3Ast {
    let mut ast = Vue3Dialect::base_parse(source, &options.core);
    normalize_dom_ast(&mut ast, options);
    ast
}

pub fn compile(source: TemplateSource, options: DomCompilerOptions) -> CodegenResult {
    let mut ast = parse(source.clone(), &options);
    let mut ctx = TransformContext::default();
    remove_side_effect_nodes(&mut ast, &mut ctx);
    report_transition_invalid_children(&ast, &mut ctx);
    for node in &mut ast.nodes {
        if let Vue3AstKind::Element(element) = &mut node.kind {
            let tag = element.tag.clone();
            let mut attributes = element.template_attributes();
            let directives = extract_directives(&attributes);
            let mut summaries = Vec::new();
            for directive in directives {
                match directive.name.as_str() {
                    "html" => summaries.push("v-html".to_string()),
                    "text" => summaries.push("v-text".to_string()),
                    "show" => summaries.push("v-show".to_string()),
                    "model" => summaries.push(format!(
                        "v-model:{}",
                        model_runtime_helper(&tag, &directive)
                    )),
                    "on" => summaries.push(format!("v-on:{}", directive.modifiers.join("."))),
                    "bind" => summaries.push(format!("v-bind:{}", directive.modifiers.join("."))),
                    _ => summaries.push(format!("v-{}", directive.name)),
                }
            }
            if options.transform_asset_urls {
                summaries.extend(asset_url_attributes(&tag, &attributes));
            }
            if !summaries.is_empty() && !only_asset_summaries(&summaries) {
                attributes.push(TemplateAttribute {
                    name: "data-vuec-dom".into(),
                    value: Some(summaries.join(",")),
                });
            }
            element.props = attributes.into_iter().map(Vue3Prop::from).collect();
        }
    }
    Vue3Dialect::transform(&mut ast, &mut ctx);
    let mut result = Vue3Dialect::finish_compile(ast, source, options.core, ctx);
    result.ast_summary = format!("dom:{}", result.ast_summary);
    result
}

pub fn normalize_dom_ast(ast: &mut Vue3Ast, options: &DomCompilerOptions) {
    for node in &mut ast.nodes {
        match &mut node.kind {
            Vue3AstKind::Text(text) if options.decode_entities => {
                text.value = decode_basic_entities(&text.value);
            }
            Vue3AstKind::Element(element) => {
                if options
                    .is_custom_element
                    .iter()
                    .any(|custom| custom == &element.tag)
                {
                    let mut attributes = element.template_attributes();
                    attributes.push(TemplateAttribute {
                        name: "data-vuec-custom-element".into(),
                        value: None,
                    });
                    element.props = attributes.into_iter().map(Vue3Prop::from).collect();
                }
            }
            _ => {}
        }
    }
}

pub fn transform_style_projection(payload: &Value) -> Value {
    let props = payload
        .get("node")
        .and_then(|node| node.get("props"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let replacements = props
        .iter()
        .enumerate()
        .filter_map(|(index, prop)| {
            let is_static_style = prop.get("type").and_then(Value::as_u64) == Some(6)
                && prop.get("name").and_then(Value::as_str) == Some("style")
                && prop.get("value").is_some_and(|value| !value.is_null());
            if !is_static_style {
                return None;
            }
            let value = prop
                .get("value")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            Some(json!({
                "index": index,
                "expression": style_json_string(value),
            }))
        })
        .collect::<Vec<_>>();
    json!({ "replacements": replacements })
}

pub fn extract_directives(attributes: &[TemplateAttribute]) -> Vec<DomDirective> {
    attributes
        .iter()
        .filter_map(|attr| parse_directive(attr))
        .collect()
}

fn style_json_string(value: &str) -> String {
    let style = parse_string_style(value);
    let properties = style
        .iter()
        .map(|(name, value)| {
            format!(
                "{}:{}",
                serde_json::to_string(name).unwrap_or_else(|_| "\"\"".into()),
                serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{properties}}}")
}

fn parse_string_style(value: &str) -> Vec<(String, String)> {
    let mut style = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in strip_css_comments(value).chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ';' if depth == 0 => {
                push_style_declaration(&mut style, &current);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    push_style_declaration(&mut style, &current);
    style
}

fn strip_css_comments(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn push_style_declaration(style: &mut Vec<(String, String)>, item: &str) {
    let Some((name, value)) = item.split_once(':') else {
        return;
    };
    let name = name.trim();
    let value = value.trim();
    if !name.is_empty() && !value.is_empty() {
        if let Some((_, existing)) = style.iter_mut().find(|(existing, _)| existing == name) {
            *existing = value.to_string();
        } else {
            style.push((name.to_string(), value.to_string()));
        }
    }
}

fn parse_directive(attr: &TemplateAttribute) -> Option<DomDirective> {
    let raw = attr.name.as_str();
    let (name, tail) = if let Some(stripped) = raw.strip_prefix("v-") {
        stripped
            .split_once(':')
            .map_or((stripped, ""), |(name, tail)| (name, tail))
    } else if let Some(argument) = raw.strip_prefix('@') {
        ("on", argument)
    } else if let Some(argument) = raw.strip_prefix(':') {
        ("bind", argument)
    } else {
        return None;
    };
    let mut parts = tail.split('.').filter(|part| !part.is_empty());
    let argument = parts.next().map(str::to_string);
    let modifiers = parts.map(str::to_string).collect();
    Some(DomDirective {
        name: name.to_string(),
        argument,
        modifiers,
        expression: attr.value.clone(),
    })
}

fn model_runtime_helper(tag: &str, directive: &DomDirective) -> &'static str {
    if tag == "select" {
        "vModelSelect"
    } else if tag == "textarea" {
        "vModelText"
    } else if directive.argument.as_deref() == Some("type") {
        "vModelDynamic"
    } else {
        match directive.expression.as_deref() {
            Some(value) if value.contains("checkbox") => "vModelCheckbox",
            Some(value) if value.contains("radio") => "vModelRadio",
            _ => "vModelText",
        }
    }
}

fn asset_url_attributes(tag: &str, attributes: &[TemplateAttribute]) -> Vec<String> {
    let mut found = Vec::new();
    for attr in attributes {
        let is_asset = matches!(
            (tag, attr.name.as_str()),
            ("img", "src")
                | ("img", "srcset")
                | ("source", "src")
                | ("source", "srcset")
                | ("video", "poster")
        );
        if is_asset {
            found.push(format!("asset:{}", attr.name));
        }
    }
    found
}

fn only_asset_summaries(summaries: &[String]) -> bool {
    summaries
        .iter()
        .all(|summary| summary.starts_with("asset:"))
}

fn report_transition_invalid_children(ast: &Vue3Ast, ctx: &mut TransformContext) {
    report_transition_invalid_children_for_node(ast, ast.root, ctx);
}

fn report_transition_invalid_children_for_node(
    ast: &Vue3Ast,
    node_id: NodeId,
    ctx: &mut TransformContext,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        if element.tag_type == Vue3ElementType::Component
            && matches!(element.tag.as_str(), "Transition" | "transition")
            && transition_children_are_invalid(ast, &node.children)
        {
            ctx.report(Diagnostic {
                code: "63".into(),
                severity: Severity::Error,
                message: "<Transition> expects exactly one child element or component.".into(),
                span: node.span.source(),
                notes: Vec::new(),
            });
        }
    }
    for child_id in node.children.clone() {
        report_transition_invalid_children_for_node(ast, child_id, ctx);
    }
}

fn transition_children_are_invalid(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    if children.is_empty() {
        return false;
    }
    transition_child_sequence_is_invalid(ast, &transition_visible_child_ids(ast, children), false)
}

fn transition_child_sequence_is_invalid(
    ast: &Vue3Ast,
    visible_children: &[NodeId],
    empty_is_invalid: bool,
) -> bool {
    if visible_children.is_empty() {
        return empty_is_invalid;
    }
    let mut logical_children = 0usize;
    let mut invalid = false;
    let mut index = 0usize;
    while index < visible_children.len() {
        logical_children += 1;
        let child_id = visible_children[index];
        if transition_child_starts_if_chain(ast, child_id) {
            let (branches, next_index) = collect_transition_if_chain(ast, visible_children, index);
            invalid |= branches
                .iter()
                .any(|branch_id| transition_if_branch_is_invalid(ast, *branch_id));
            index = next_index;
        } else {
            invalid |= transition_single_child_is_invalid(ast, child_id);
            index += 1;
        }
    }
    logical_children != 1 || invalid
}

fn transition_single_child_is_invalid(ast: &Vue3Ast, child_id: NodeId) -> bool {
    let Some(child) = ast.node(child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    element_has_directive(element, "for")
}

fn transition_if_branch_is_invalid(ast: &Vue3Ast, branch_id: NodeId) -> bool {
    let Some(branch) = ast.node(branch_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &branch.kind else {
        return false;
    };
    if element_has_directive(element, "for") {
        return true;
    }
    if element.tag == "template" {
        return transition_child_sequence_is_invalid(
            ast,
            &transition_visible_child_ids(ast, &branch.children),
            true,
        );
    }
    false
}

fn collect_transition_if_chain(
    ast: &Vue3Ast,
    visible_children: &[NodeId],
    start: usize,
) -> (Vec<NodeId>, usize) {
    let mut branches = vec![visible_children[start]];
    let mut index = start + 1;
    while index < visible_children.len() {
        let Some(node) = ast.node(visible_children[index]) else {
            index += 1;
            continue;
        };
        let Vue3AstKind::Element(element) = &node.kind else {
            break;
        };
        if element_has_directive(element, "else-if") || element_has_directive(element, "else") {
            branches.push(visible_children[index]);
            index += 1;
        } else {
            break;
        }
    }
    (branches, index)
}

fn transition_child_starts_if_chain(ast: &Vue3Ast, child_id: NodeId) -> bool {
    ast.node(child_id).is_some_and(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element) if element_has_directive(element, "if")
        )
    })
}

fn transition_visible_child_ids(ast: &Vue3Ast, children: &[NodeId]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|child_id| {
            ast.node(*child_id).is_some_and(|child| match &child.kind {
                Vue3AstKind::Comment(_) => false,
                Vue3AstKind::Text(text) => !text.value.chars().all(is_html_whitespace),
                _ => true,
            })
        })
        .collect()
}

fn element_has_directive(element: &Vue3Element, name: &str) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(directive) if directive.name == name
        )
    })
}

fn is_html_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

fn remove_side_effect_nodes(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
    remove_side_effect_children(ast, ast.root, ctx);
}

fn remove_side_effect_children(ast: &mut Vue3Ast, parent_id: NodeId, ctx: &mut TransformContext) {
    let child_ids = ast
        .node(parent_id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut retained = Vec::new();
    for child_id in child_ids {
        let remove = ast.node(child_id).is_some_and(|child| {
            matches!(
                child.kind,
                Vue3AstKind::Element(ref element) if element.tag == "script" || element.tag == "style"
            )
        });
        if remove {
            if let Some(span) = ast.node(child_id).and_then(|node| node.span.source()) {
                ctx.report(Diagnostic {
                    code: "64".into(),
                    severity: Severity::Error,
                    message: "Tags with side effect (<script> and <style>) are ignored in client component templates.".into(),
                    span: Some(span),
                    notes: Vec::new(),
                });
            }
        } else {
            remove_side_effect_children(ast, child_id, ctx);
            retained.push(child_id);
        }
    }
    if let Some(parent) = ast.node_mut(parent_id) {
        parent.children = retained;
    }
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vuec_source::FileId;

    #[test]
    fn extracts_dom_directives() {
        let attrs = vec![
            TemplateAttribute {
                name: "@click.stop".into(),
                value: Some("save".into()),
            },
            TemplateAttribute {
                name: "v-model".into(),
                value: Some("checked".into()),
            },
        ];
        let directives = extract_directives(&attrs);
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[0].name, "on");
        assert_eq!(directives[0].modifiers, vec!["stop"]);
    }

    #[test]
    fn compile_records_dom_summary() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./a.png"><input v-model="value">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions::default(),
        );
        assert!(result.ast_summary.starts_with("dom:"));
        assert!(result.code.contains("data-vuec-dom"));
    }

    #[test]
    fn parse_marks_dom_transition_builtins() {
        let ast = parse(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<transition/><transition-group/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            &DomCompilerOptions::default(),
        );
        let tags = ast
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3AstKind::Element(element) => Some((
                    element.tag.as_str(),
                    element.tag_type == vuec_ast::Vue3ElementType::Component,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(tags, vec![("transition", true), ("transition-group", true)]);
    }

    #[test]
    fn compile_reports_transition_invalid_children_diagnostics() {
        let cases = [
            ("<transition><div>hey</div><div>hey</div></transition>", true),
            ("<transition><div v-for=\"i in items\">hey</div></transition>", true),
            (
                "<transition><div v-if=\"a\" v-for=\"i in items\">hey</div><div v-else v-for=\"i in items\">hey</div></transition>",
                true,
            ),
            ("<transition><template v-if=\"ok\"></template></transition>", true),
            (
                "<transition><template v-if=\"a\"></template><template v-else></template></transition>",
                true,
            ),
            (
                "<transition><div v-if=\"one\">hey</div><div v-if=\"other\">hey</div></transition>",
                true,
            ),
            ("<transition><div>hey</div></transition>", false),
            ("<transition><div v-if=\"a\">hey</div></transition>", false),
            (
                "<transition><div v-if=\"a\">hey</div><div v-else-if=\"b\">hey</div><div v-else>hey</div></transition>",
                false,
            ),
            (
                "<transition><div v-if=\"a\">hey</div><div v-else>hey</div></transition>",
                false,
            ),
            ("<transition>\u{00a0}<div>foo</div></transition>", true),
            (
                "<transition><!-- foo --> <!-- bar --><div>foo bar</div></transition>",
                false,
            ),
        ];
        for (index, (source, should_warn)) in cases.iter().enumerate() {
            let result = compile(
                TemplateSource {
                    filename: format!("case-{index}.vue"),
                    source: (*source).into(),
                    file_id: FileId(index as u32),
                    base_offset: 0,
                },
                DomCompilerOptions::default(),
            );

            let has_warning = result.diagnostics.iter().any(|diagnostic| {
                diagnostic == "<Transition> expects exactly one child element or component."
            });
            assert_eq!(has_warning, *should_warn, "case {index}: {source}");
        }
    }

    #[test]
    fn transform_style_projection_rewrites_static_style() {
        let projection = transform_style_projection(&json!({
            "node": {
                "props": [
                    {
                        "type": 6,
                        "name": "style",
                        "value": {
                            "content": "color: green; background: url(a;b); /* x */ margin: 0"
                        }
                    }
                ]
            }
        }));

        assert_eq!(projection["replacements"][0]["index"], json!(0));
        assert_eq!(
            projection["replacements"][0]["expression"],
            json!("{\"color\":\"green\",\"background\":\"url(a;b)\",\"margin\":\"0\"}")
        );
    }
}
