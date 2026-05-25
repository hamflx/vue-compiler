#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vuec_ast::{TemplateAttribute, Vue3Expression, Vue3ImportItem, Vue3Prop};

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

pub fn default_asset_url_tags() -> BTreeMap<String, Vec<String>> {
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

pub fn transform_asset_url_props(
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

pub fn asset_url_attributes(
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
