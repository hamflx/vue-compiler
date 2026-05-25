#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use vuec_ast::{
    NodeId, TemplateAttribute, Vue3Ast, Vue3AstKind, Vue3Element, Vue3ElementType, Vue3Expression,
    Vue3ImportItem, Vue3Prop, Vue3Root,
};
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_pass::TransformContext;
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomCompilerOptions {
    pub core: Vue3CompilerOptions,
    pub is_custom_element: Vec<String>,
    pub transform_asset_urls: bool,
    pub asset_url_options: AssetUrlOptions,
    pub decode_entities: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetUrlOptions {
    pub base: Option<String>,
    pub include_absolute: bool,
    pub tags: BTreeMap<String, Vec<String>>,
}

impl Default for AssetUrlOptions {
    fn default() -> Self {
        Self {
            base: None,
            include_absolute: false,
            tags: default_asset_url_tags(),
        }
    }
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
            asset_url_options: AssetUrlOptions::default(),
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

fn default_asset_url_tags() -> BTreeMap<String, Vec<String>> {
    [
        ("video", vec!["src", "poster"]),
        ("source", vec!["src", "srcset"]),
        ("img", vec!["src", "srcset"]),
        ("image", vec!["xlink:href", "href"]),
        ("use", vec!["xlink:href", "href"]),
    ]
    .into_iter()
    .map(|(tag, attrs)| {
        (
            tag.to_string(),
            attrs.into_iter().map(str::to_string).collect(),
        )
    })
    .collect()
}

fn transform_asset_url_props(
    tag: &str,
    props: &mut [Vue3Prop],
    options: &AssetUrlOptions,
    enable_imports: bool,
    imports: &mut Vec<Vue3ImportItem>,
) {
    let asset_attrs = asset_url_attrs_for_tag(tag, options);
    let base = options.base.as_deref().filter(|base| !base.is_empty());
    for prop in props {
        let Vue3Prop::Attribute(attr) = prop else {
            continue;
        };
        let is_srcset = attr.name == "srcset" && matches!(tag, "img" | "source");
        if !is_srcset && !asset_attrs.iter().any(|candidate| candidate == &attr.name) {
            continue;
        }
        let Some(value) = attr.value.clone() else {
            continue;
        };
        let value_span = attr.value_span.or(attr.span);
        if is_srcset {
            if let Some(base) = base {
                if let Some(rewritten) = rewrite_srcset_base(&value, base, options) {
                    attr.value = Some(rewritten);
                    continue;
                }
            }
            if enable_imports {
                if let Some(expression) = asset_srcset_import_expression(&value, options, imports) {
                    *prop = asset_bind_directive(
                        "srcset",
                        expression,
                        attr.span,
                        attr.name_span,
                        value_span,
                    );
                }
            }
        } else if should_process_asset_attr(tag, &attr.name, &value, options) {
            if let Some(base) = base.filter(|_| value.starts_with('.')) {
                attr.value = Some(join_asset_base(base, &value));
            } else if enable_imports {
                let expression = asset_url_import_expression(tag, &attr.name, &value, imports);
                *prop = asset_bind_directive(
                    &attr.name,
                    expression,
                    attr.span,
                    attr.name_span,
                    value_span,
                );
            }
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

fn asset_url_attrs_for_tag(tag: &str, options: &AssetUrlOptions) -> Vec<String> {
    let mut attrs = options.tags.get(tag).cloned().unwrap_or_default();
    if let Some(wildcard) = options.tags.get("*") {
        attrs.extend(wildcard.iter().cloned());
    }
    attrs
}

fn should_process_asset_attr(
    tag: &str,
    attr: &str,
    value: &str,
    options: &AssetUrlOptions,
) -> bool {
    if is_external_url(value) || is_data_url(value) || value == "#" {
        return false;
    }
    let hash_only = value.starts_with('#');
    if hash_only && !can_transform_hash_import(tag, attr) {
        return false;
    }
    options.include_absolute || is_relative_url(value)
}

fn can_transform_hash_import(tag: &str, attr: &str) -> bool {
    matches!(
        (tag, attr),
        ("video", "src" | "poster") | ("source", "src") | ("img", "src")
    )
}

fn rewrite_srcset_base(value: &str, base: &str, options: &AssetUrlOptions) -> Option<String> {
    let candidates = parse_srcset_candidates(value);
    if !candidates
        .iter()
        .any(|(url, _)| should_process_asset_url(url, options))
    {
        return None;
    }
    if candidates
        .iter()
        .any(|(url, _)| should_process_asset_url(url, options) && !url.starts_with('.'))
    {
        return None;
    }

    let mut changed = false;
    let items = candidates
        .into_iter()
        .map(|(url, descriptor)| {
            let rewritten = if url.starts_with('.') && should_process_asset_url(&url, options) {
                changed = true;
                join_asset_base(base, &url)
            } else {
                url
            };
            if descriptor.is_empty() {
                rewritten
            } else {
                format!("{rewritten} {descriptor}")
            }
        })
        .collect::<Vec<_>>();
    if changed {
        Some(items.join(", "))
    } else {
        None
    }
}

fn asset_url_import_expression(
    tag: &str,
    attr: &str,
    value: &str,
    imports: &mut Vec<Vue3ImportItem>,
) -> String {
    let (path, hash) = parse_asset_url(value);
    import_expression_from_parts(tag, attr, &path, &hash, imports)
}

fn asset_srcset_import_expression(
    value: &str,
    options: &AssetUrlOptions,
    imports: &mut Vec<Vue3ImportItem>,
) -> Option<String> {
    let candidates = parse_srcset_candidates(value);
    if !candidates
        .iter()
        .any(|(url, _)| should_process_asset_url(url, options))
    {
        return None;
    }

    let mut parts = Vec::<String>::new();
    for (index, (url, descriptor)) in candidates.iter().enumerate() {
        let mut item = if should_process_asset_url(url, options) {
            let (path, hash) = parse_asset_url(url);
            import_expression_from_parts("img", "src", &path, &hash, imports)
        } else {
            quote_js_double_string(url)
        };
        let is_not_last = index + 1 < candidates.len();
        if !descriptor.is_empty() && is_not_last {
            item = format!(
                "{item} + {}",
                quote_js_single_string(&format!(" {descriptor}, "))
            );
        } else if !descriptor.is_empty() {
            item = format!(
                "{item} + {}",
                quote_js_single_string(&format!(" {descriptor}"))
            );
        } else if is_not_last {
            item = format!("{item} + {}", quote_js_single_string(", "));
        }
        parts.push(item);
    }
    (!parts.is_empty()).then(|| parts.join(" + "))
}

fn import_expression_from_parts(
    tag: &str,
    attr: &str,
    path: &str,
    hash: &str,
    imports: &mut Vec<Vue3ImportItem>,
) -> String {
    if path.is_empty() && hash.is_empty() {
        return "''".into();
    }
    let source = if path.is_empty() && !hash.is_empty() {
        hash
    } else {
        path
    };
    let name = register_asset_import(source, imports);
    if !path.is_empty() && !hash.is_empty() {
        format!("{name} + {}", quote_js_single_string(hash))
    } else if path.is_empty() && !hash.is_empty() && !can_transform_hash_import(tag, attr) {
        quote_js_single_string(hash)
    } else {
        name
    }
}

fn register_asset_import(source: &str, imports: &mut Vec<Vue3ImportItem>) -> String {
    let normalized = normalize_decoded_import_path(source);
    if let Some(index) = imports.iter().position(|import| import.path == normalized) {
        return format!("_imports_{index}");
    }
    let name = format!("_imports_{}", imports.len());
    imports.push(Vue3ImportItem {
        name: name.clone(),
        path: normalized,
    });
    name
}

fn normalize_decoded_import_path(source: &str) -> String {
    percent_decode_lossless(source)
}

fn percent_decode_lossless(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(high) = bytes.get(index + 1).and_then(|byte| hex_value(*byte)) else {
                return source.to_string();
            };
            let Some(low) = bytes.get(index + 2).and_then(|byte| hex_value(*byte)) else {
                return source.to_string();
            };
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).unwrap_or_else(|_| source.to_string())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn quote_js_single_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("'{escaped}'")
}

fn quote_js_double_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn asset_bind_directive(
    name: &str,
    expression: String,
    span: Option<vuec_source::Span>,
    name_span: Option<vuec_source::Span>,
    exp_span: Option<vuec_source::Span>,
) -> Vue3Prop {
    Vue3Prop::Directive(vuec_ast::Vue3Directive {
        name: "bind".into(),
        raw_name: format!(":{name}"),
        arg: Some(Vue3Expression::Raw(name.to_string())),
        exp: Some(Vue3Expression::Raw(expression)),
        modifiers: Vec::new(),
        is_dynamic_arg: false,
        span,
        arg_span: name_span,
        exp_span,
        modifier_spans: Vec::new(),
    })
}

fn should_process_asset_url(url: &str, options: &AssetUrlOptions) -> bool {
    !url.is_empty()
        && !is_external_url(url)
        && !is_data_url(url)
        && (options.include_absolute || is_relative_url(url))
}

fn parse_srcset_candidates(value: &str) -> Vec<(String, String)> {
    let mut candidates = Vec::<(String, String)>::new();
    for raw in value.split(',') {
        let item = raw
            .replace(['\t', '\n', '\u{000C}', '\r'], " ")
            .trim()
            .to_string();
        if item.is_empty() {
            continue;
        }
        let (url, descriptor) = item.split_once(' ').map_or_else(
            || (item.clone(), String::new()),
            |(url, descriptor)| (url.to_string(), descriptor.trim().to_string()),
        );
        candidates.push((url, descriptor));
    }

    let mut index = 0usize;
    while index + 1 < candidates.len() {
        if is_data_url(&candidates[index].0) {
            let prefix = candidates.remove(index).0;
            candidates[index].0 = format!("{prefix},{}", candidates[index].0);
        }
        index += 1;
    }
    candidates
}

fn join_asset_base(base: &str, raw_url: &str) -> String {
    let (path, hash) = parse_asset_url(raw_url);
    let normalized_path = strip_leading_dot_segments(&path);
    let (host_prefix, base_path) = split_base_host_path(base);
    let mut joined = join_url_path(base_path, &normalized_path);
    if joined.is_empty() {
        joined.push('/');
    }
    format!("{host_prefix}{joined}{hash}")
}

fn split_base_host_path(base: &str) -> (&str, &str) {
    if let Some(protocol_index) = base.find("://") {
        let after_protocol = protocol_index + 3;
        let rest = &base[after_protocol..];
        if let Some(path_index) = rest.find('/') {
            let split = after_protocol + path_index;
            return (&base[..split], &base[split..]);
        }
        return (base, "/");
    }
    if let Some(rest) = base.strip_prefix("//") {
        if let Some(path_index) = rest.find('/') {
            let split = 2 + path_index;
            return (&base[..split], &base[split..]);
        }
        return (base, "/");
    }
    ("", base)
}

fn join_url_path(base: &str, relative: &str) -> String {
    let mut parts = Vec::<&str>::new();
    let absolute = base.starts_with('/');
    for part in base.split('/').chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let mut joined = parts.join("/");
    if absolute {
        joined.insert(0, '/');
    }
    if joined.is_empty() && absolute {
        joined.push('/');
    }
    joined
}

fn strip_leading_dot_segments(value: &str) -> String {
    let mut rest = value;
    while let Some(stripped) = rest.strip_prefix("./") {
        rest = stripped;
    }
    rest.to_string()
}

fn parse_asset_url(value: &str) -> (String, String) {
    let normalized = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix('~'))
        .unwrap_or(value);
    if let Some(index) = normalized.find('#') {
        (
            normalized[..index].to_string(),
            normalized[index..].to_string(),
        )
    } else {
        (normalized.to_string(), String::new())
    }
}

fn is_relative_url(url: &str) -> bool {
    matches!(url.chars().next(), Some('.' | '~' | '@' | '#'))
}

fn is_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

fn is_data_url(url: &str) -> bool {
    url.trim_start().to_ascii_lowercase().starts_with("data:")
}

fn asset_url_attributes(
    tag: &str,
    attributes: &[TemplateAttribute],
    options: &AssetUrlOptions,
) -> Vec<String> {
    let asset_attrs = asset_url_attrs_for_tag(tag, options);
    let mut found = Vec::new();
    for attr in attributes {
        if asset_attrs.iter().any(|candidate| candidate == &attr.name)
            || (attr.name == "srcset" && matches!(tag, "img" | "source"))
        {
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
