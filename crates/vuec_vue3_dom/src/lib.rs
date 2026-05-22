#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{TemplateAttribute, Vue3Ast, Vue3NodeKind};
use vuec_source::FileId;
use vuec_vue3_core::{
    base_compile, CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomCompilerOptions {
    pub core: Vue3CompilerOptions,
    pub is_custom_element: Vec<String>,
    pub transform_asset_urls: bool,
    pub decode_entities: bool,
}

impl Default for DomCompilerOptions {
    fn default() -> Self {
        Self {
            core: Vue3CompilerOptions::default(),
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
    let mut ast = parse(source, &options);
    for node in &mut ast.nodes {
        if let Vue3NodeKind::Element {
            tag, attributes, ..
        } = &mut node.kind
        {
            let directives = extract_directives(attributes);
            let mut summaries = Vec::new();
            for directive in directives {
                match directive.name.as_str() {
                    "html" => summaries.push("v-html".to_string()),
                    "text" => summaries.push("v-text".to_string()),
                    "show" => summaries.push("v-show".to_string()),
                    "model" => {
                        summaries.push(format!("v-model:{}", model_runtime_helper(tag, &directive)))
                    }
                    "on" => summaries.push(format!("v-on:{}", directive.modifiers.join("."))),
                    "bind" => summaries.push(format!("v-bind:{}", directive.modifiers.join("."))),
                    _ => summaries.push(format!("v-{}", directive.name)),
                }
            }
            if options.transform_asset_urls {
                summaries.extend(asset_url_attributes(tag, attributes));
            }
            if !summaries.is_empty() {
                attributes.push(TemplateAttribute {
                    name: "data-vuec-dom".into(),
                    value: Some(summaries.join(",")),
                });
            }
        }
    }
    let mut result = base_compile(
        TemplateSource {
            filename: "dom.vue".into(),
            source: dom_summary_source(&ast),
            file_id: FileId(0),
        },
        options.core,
    );
    result.ast_summary = format!("dom:{}", result.ast_summary);
    result
}

pub fn normalize_dom_ast(ast: &mut Vue3Ast, options: &DomCompilerOptions) {
    for node in &mut ast.nodes {
        match &mut node.kind {
            Vue3NodeKind::Text { value } if options.decode_entities => {
                *value = decode_basic_entities(value);
            }
            Vue3NodeKind::Element {
                tag, attributes, ..
            } => {
                if options.is_custom_element.iter().any(|custom| custom == tag) {
                    attributes.push(TemplateAttribute {
                        name: "data-vuec-custom-element".into(),
                        value: None,
                    });
                }
            }
            _ => {}
        }
    }
}

pub fn extract_directives(attributes: &[TemplateAttribute]) -> Vec<DomDirective> {
    attributes
        .iter()
        .filter_map(|attr| parse_directive(attr))
        .collect()
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

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn dom_summary_source(ast: &Vue3Ast) -> String {
    let mut source = String::new();
    for node in &ast.nodes {
        match &node.kind {
            Vue3NodeKind::Element {
                tag,
                attributes,
                self_closing,
            } => {
                source.push('<');
                source.push_str(tag);
                for attr in attributes {
                    source.push(' ');
                    source.push_str(&attr.name);
                    if let Some(value) = &attr.value {
                        source.push_str("=\"");
                        source.push_str(value);
                        source.push('"');
                    }
                }
                if *self_closing {
                    source.push_str("/>");
                } else {
                    source.push('>');
                }
            }
            Vue3NodeKind::Text { value } => source.push_str(value),
            Vue3NodeKind::Interpolation { expression } => {
                source.push_str("{{");
                source.push_str(expression);
                source.push_str("}}");
            }
            Vue3NodeKind::Comment { value } => {
                source.push_str("<!--");
                source.push_str(value);
                source.push_str("-->");
            }
            Vue3NodeKind::Directive { .. } | Vue3NodeKind::Root => {}
        }
    }
    source
}

#[cfg(test)]
mod tests {
    use super::*;

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
            },
            DomCompilerOptions::default(),
        );
        assert!(result.ast_summary.starts_with("dom:"));
        assert!(result.code.contains("data-vuec-dom"));
    }
}
