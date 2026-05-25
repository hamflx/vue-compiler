#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    NodeId, TemplateAttribute, Vue3Ast, Vue3AstKind, Vue3Element, Vue3ElementType, Vue3ImportItem,
    Vue3Prop, Vue3Root,
};
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_pass::TransformContext;
pub use vuec_vue3_asset::AssetUrlOptions;
use vuec_vue3_asset::{asset_url_attributes, transform_asset_url_props};
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomCompilerOptions {
    pub core: Vue3CompilerOptions,
    pub is_custom_element: Vec<String>,
    pub transform_asset_urls: bool,
    pub asset_url_options: AssetUrlOptions,
    pub decode_entities: bool,
}

impl Default for DomCompilerOptions {
    fn default() -> Self {
        let mut core = Vue3CompilerOptions::default();
        apply_dom_parser_defaults(&mut core);
        core.dom_namespaces = true;
        Self {
            core,
            is_custom_element: Vec::new(),
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
            decode_entities: true,
        }
    }
}

pub fn apply_dom_parser_defaults(core: &mut Vue3CompilerOptions) {
    if core.void_tags.is_empty() {
        core.void_tags = DOM_VOID_TAGS.iter().map(|tag| (*tag).to_string()).collect();
    }
    if core.native_tags.is_none() {
        core.native_tags = Some(
            DOM_HTML_TAGS
                .iter()
                .chain(DOM_SVG_TAGS.iter())
                .chain(DOM_MATH_TAGS.iter())
                .map(|tag| (*tag).to_string())
                .collect(),
        );
    }
    if core.pre_tags.is_empty() {
        core.pre_tags = vec!["pre".into()];
    }
    if core.ignore_newline_tags.is_empty() {
        core.ignore_newline_tags = vec!["pre".into(), "textarea".into()];
    }
    core.dom_namespaces = true;
    if core.built_in_components.is_empty() {
        core.built_in_components = vec![
            "Transition".into(),
            "transition".into(),
            "TransitionGroup".into(),
            "transition-group".into(),
        ];
    }
}

const DOM_VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const DOM_HTML_TAGS: &[&str] = &[
    "html",
    "body",
    "base",
    "head",
    "link",
    "meta",
    "style",
    "title",
    "address",
    "article",
    "aside",
    "footer",
    "header",
    "hgroup",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "nav",
    "section",
    "div",
    "dd",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "picture",
    "hr",
    "img",
    "li",
    "main",
    "ol",
    "p",
    "pre",
    "ul",
    "a",
    "b",
    "abbr",
    "bdi",
    "bdo",
    "br",
    "cite",
    "code",
    "data",
    "dfn",
    "em",
    "i",
    "kbd",
    "mark",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "small",
    "span",
    "strong",
    "sub",
    "sup",
    "time",
    "u",
    "var",
    "wbr",
    "area",
    "audio",
    "map",
    "track",
    "video",
    "embed",
    "object",
    "param",
    "source",
    "canvas",
    "script",
    "noscript",
    "del",
    "ins",
    "caption",
    "col",
    "colgroup",
    "table",
    "thead",
    "tbody",
    "td",
    "th",
    "tr",
    "button",
    "datalist",
    "fieldset",
    "form",
    "input",
    "label",
    "legend",
    "meter",
    "optgroup",
    "option",
    "output",
    "progress",
    "select",
    "textarea",
    "details",
    "dialog",
    "menu",
    "summary",
    "template",
    "blockquote",
    "iframe",
    "tfoot",
];

const DOM_SVG_TAGS: &[&str] = &[
    "svg",
    "animate",
    "animateMotion",
    "animateTransform",
    "circle",
    "clipPath",
    "color-profile",
    "defs",
    "desc",
    "discard",
    "ellipse",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDistantLight",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feImage",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "fePointLight",
    "feSpecularLighting",
    "feSpotLight",
    "feTile",
    "feTurbulence",
    "filter",
    "foreignObject",
    "g",
    "hatch",
    "hatchpath",
    "image",
    "line",
    "linearGradient",
    "marker",
    "mask",
    "mesh",
    "meshgradient",
    "meshpatch",
    "meshrow",
    "metadata",
    "mpath",
    "path",
    "pattern",
    "polygon",
    "polyline",
    "radialGradient",
    "rect",
    "set",
    "solidcolor",
    "stop",
    "switch",
    "symbol",
    "text",
    "textPath",
    "title",
    "tspan",
    "unknown",
    "use",
    "view",
];

const DOM_MATH_TAGS: &[&str] = &[
    "annotation",
    "annotation-xml",
    "maction",
    "maligngroup",
    "malignmark",
    "math",
    "menclose",
    "merror",
    "mfenced",
    "mfrac",
    "mfraction",
    "mglyph",
    "mi",
    "mlabeledtr",
    "mlongdiv",
    "mmultiscripts",
    "mn",
    "mo",
    "mover",
    "mpadded",
    "mphantom",
    "mprescripts",
    "mroot",
    "mrow",
    "ms",
    "mscarries",
    "mscarry",
    "msgroup",
    "msline",
    "mspace",
    "msqrt",
    "msrow",
    "mstack",
    "mstyle",
    "msub",
    "msubsup",
    "msup",
    "mtable",
    "mtd",
    "mtext",
    "mtr",
    "munder",
    "munderover",
    "none",
    "semantics",
];

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
    let mut asset_imports = Vec::<Vue3ImportItem>::new();
    for node_index in 0..ast.nodes.len() {
        if let Vue3AstKind::Element(element) = &mut ast.nodes[node_index].kind {
            let tag = element.tag.clone();
            let source_attributes = element.template_attributes();
            if options.transform_asset_urls {
                transform_asset_url_props(
                    &tag,
                    &mut element.props,
                    &options.asset_url_options,
                    options.core.mode == "module",
                    &mut asset_imports,
                );
            }
            let directives = extract_directives(&source_attributes);
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
                summaries.extend(asset_url_attributes(
                    &tag,
                    &source_attributes,
                    &options.asset_url_options,
                ));
            }
            if !summaries.is_empty() && !only_asset_summaries(&summaries) {
                element.props.push(Vue3Prop::from(TemplateAttribute {
                    name: "data-vuec-dom".into(),
                    value: Some(summaries.join(",")),
                }));
            }
        }
    }
    if !asset_imports.is_empty() {
        if let Some(root) = vue3_dom_root_mut(&mut ast) {
            root.imports = asset_imports;
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

fn vue3_dom_root_mut(ast: &mut Vue3Ast) -> Option<&mut Vue3Root> {
    let root = ast.root_node_mut()?;
    match &mut root.kind {
        Vue3AstKind::Root(root) => Some(root),
        _ => None,
    }
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
    use std::collections::BTreeMap;
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
    fn parse_uses_dom_parser_defaults() {
        let ast = parse(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<input><hello/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            &DomCompilerOptions::default(),
        );
        let root = ast.node(ast.root).expect("root");
        let input = ast.node(root.children[0]).expect("input");
        let hello = ast.node(root.children[1]).expect("hello");

        assert!(input.children.is_empty());
        assert!(matches!(
            &hello.kind,
            Vue3AstKind::Element(element)
                if element.tag == "hello" && element.tag_type == Vue3ElementType::Component
        ));
    }

    #[test]
    fn compile_rewrites_static_asset_urls_with_explicit_base_without_module_imports() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./bar.png"><img src="bar.png"><img src="~bar.png"><img src="@theme/bar.png"><img src="/bar.png"><img src="data:image/png;base64,i">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "/foo/bar.png""#));
        assert!(result.code.contains(r#"src: "bar.png""#));
        assert!(result.code.contains(r#"src: "~bar.png""#));
        assert!(result.code.contains(r#"src: "@theme/bar.png""#));
        assert!(result.code.contains(r#"src: "/bar.png""#));
        assert!(result.code.contains(r#"src: "data:image/png;base64,i""#));
        assert!(!result.code.contains("_imports_"));
        assert!(!result.code.contains("import _imports_"));
    }

    #[test]
    fn compile_transforms_asset_urls_to_imports_in_module_mode() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r##"<img src="./bar.png"><img src="~fixtures/logo.png"><img src="@theme/bar.png"><img src="./icons.svg#heart"><use href="#local"></use>"##.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result
            .code
            .contains("import _imports_1 from 'fixtures/logo.png'"));
        assert!(result
            .code
            .contains("import _imports_2 from '@theme/bar.png'"));
        assert!(result.code.contains("import _imports_3 from './icons.svg'"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("src: _imports_1"));
        assert!(result.code.contains("src: _imports_2"));
        assert!(result.code.contains(r#"src: _imports_3 + '#heart'"#));
        assert!(result.code.contains(r##"href: "#local""##));
        assert!(!result.code.contains("_ctx._imports_"));
    }

    #[test]
    fn compile_caches_static_children_with_asset_url_imports() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<div><img src="./bar.png"><span title="static">ok</span></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("-1"));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("8 /* PROPS */"));
        assert!(!result.code.contains("[\"src\"]"));
    }

    #[test]
    fn compile_stringifies_static_children_with_asset_url_imports() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><img src="./bar.png" srcset="./bar.png, ./icons.svg#heart 2x" />{}</div>"#,
                    r#"<span title="static">ok</span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(
            result.code.contains("_createStaticVNode"),
            "{}",
            result.code
        );
        assert!(result.code.contains(r##"_createStaticVNode("<img src=\"" + _imports_0 + "\" srcset=\"" + _imports_0 + ", " + _imports_1 + "#heart 2x\"><span title=\"static\">ok</span>"##));
        assert!(!result.code.contains("src: _imports_0"));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("_ctx._imports_1"));
    }

    #[test]
    fn compile_stringifies_multiple_static_chunks_around_dynamic_child() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    "<div>{}{{{{ msg }}}}{}</div>",
                    r#"<span class="foo"></span>"#.repeat(5),
                    r#"<span class="bar"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert_eq!(result.code.matches("_createStaticVNode(").count(), 2);
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span>\", 5)"));
    }

    #[test]
    fn compile_bails_stringify_static_invalid_p_child_placement() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                        + "<span><div class=\"block\"></div></span>"
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("_createElementVNode(\"p\""));
        assert!(result
            .code
            .contains("_createElementVNode(\"div\", { class: \"block\" })"));
    }

    #[test]
    fn compile_stringifies_static_children_when_transform_hoist_requested() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn compile_stringifies_static_constant_bindings_when_transform_hoist_requested() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><div :style="`color:red;`">{}</div></div>"#,
                    r#"<span :class="[{ foo: true }, { bar: true }]">{{ 1 }} + {{ false }}</span>"#
                        .repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result.code.contains(
            r#"<div style=\"color:red;\" data-vuec-dom=\"v-bind:\"><span class=\"foo bar\" data-vuec-dom=\"v-bind:\">1 + false</span>"#
        ));
    }

    #[test]
    fn compile_stringifies_static_children_with_scope_id() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.mode = "module".into();
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        options.core.scope_id = Some("data-v-test".into());
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><div :style="`color:red;`">{}</div></div>"#,
                    r#"<span class="foo">ok</span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(
            r#"<div style=\"color:red;\" data-vuec-dom=\"v-bind:\" data-v-test><span class=\"foo\" data-v-test>ok</span>"#
        ));
    }

    #[test]
    fn compile_stringifies_static_svg_namespace_children_by_default() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><svg width="50" height="50" viewBox="0 0 50 50" fill="none" xmlns="http://www.w3.org/2000/svg">{}</svg></div>"#,
                    r##"<rect width="50" height="50" fill="#C4C4C4"></rect>"##.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(r#"<svg width=\"50\" height=\"50\" viewBox=\"0 0 50 50\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"#));
        assert!(result
            .code
            .contains(r##"<rect width=\"50\" height=\"50\" fill=\"#C4C4C4\"></rect>"##));
    }

    #[test]
    fn compile_transforms_srcset_imports_in_module_mode() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png, ./icons.svg#heart 2x, /absolute.png 3x, data:image/png;base64,i 4x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(result.code.matches("import _imports_0").count() == 1);
        assert!(result.code.contains(
            r#"srcset: _imports_0 + ', ' + _imports_1 + '#heart' + ' 2x, ' + "/absolute.png" + ' 3x, ' + "data:image/png;base64,i" + ' 4x'"#
        ));
    }

    #[test]
    fn compile_rewrites_asset_url_base_with_hosts_and_hashes() {
        let cases = [
            (
                "http://localhost:3000/src/",
                "./logo.png",
                r#"src: "http://localhost:3000/src/logo.png""#,
            ),
            (
                "http://localhost:3000",
                "./logo.png",
                r#"src: "http://localhost:3000/logo.png""#,
            ),
            (
                "http://localhost",
                "./logo.png",
                r#"src: "http://localhost/logo.png""#,
            ),
            (
                "//localhost",
                "./logo.png",
                r#"src: "//localhost/logo.png""#,
            ),
            (
                "/foo",
                "./icons.svg#heart",
                r#"src: "/foo/icons.svg#heart""#,
            ),
        ];

        for (index, (base, url, expected)) in cases.iter().enumerate() {
            let result = compile(
                TemplateSource {
                    filename: format!("asset-base-{index}.vue"),
                    source: format!(r#"<img src="{url}">"#),
                    file_id: FileId(index as u32),
                    base_offset: 0,
                },
                DomCompilerOptions {
                    asset_url_options: AssetUrlOptions {
                        base: Some((*base).into()),
                        ..AssetUrlOptions::default()
                    },
                    ..DomCompilerOptions::default()
                },
            );

            assert!(
                result.code.contains(expected),
                "base {base} url {url} generated:\n{}",
                result.code
            );
        }
    }

    #[test]
    fn compile_rewrites_srcset_base_when_all_processable_urls_are_dot_relative() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img srcset="./logo.png, ./logo.png 2x, /logo.png 3x, data:image/png;base64,i 4x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            r#"srcset: "/foo/logo.png, /foo/logo.png 2x, /logo.png 3x, data:image/png;base64,i 4x""#
        ));
    }

    #[test]
    fn compile_rewrites_srcset_base_independently_of_asset_tag_options() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    tags: BTreeMap::new(),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "./logo.png""#));
        assert!(result.code.contains(r#"srcset: "/foo/logo.png 2x""#));
    }

    #[test]
    fn compile_leaves_mixed_import_srcset_unchanged_for_base_slice() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img srcset="@/logo.png, ./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result
            .code
            .contains(r#"srcset: "@/logo.png, ./logo.png 2x""#));
    }

    #[test]
    fn compile_respects_disabled_asset_url_transform() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./bar.png">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                transform_asset_urls: false,
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "./bar.png""#));
        assert!(!result.code.contains("/foo/bar.png"));
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
