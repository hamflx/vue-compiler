//! Vue 3 template asset URL transformation helpers.
//!
//! These functions rewrite static asset attributes into import-backed `v-bind`
//! props or base-prefixed URLs, matching the compiler-core transform boundary.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vuec_ast::{TemplateAttribute, Vue3Expression, Vue3ImportItem, Vue3Prop};

/// Options controlling Vue 3 asset URL transforms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetUrlOptions {
    /// Optional public base used to rewrite dot-relative URLs.
    pub base: Option<String>,
    /// Whether root-absolute URLs should be eligible for import transforms.
    pub include_absolute: bool,
    /// Map of tag name to asset-bearing attribute names.
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

/// Returns Vue 3's default tag-to-asset-attribute map.
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

/// Rewrites asset URL attributes in-place and registers generated imports.
///
/// When `enable_imports` is true, eligible static URLs are converted into
/// `v-bind` directives whose expressions reference entries pushed into
/// `imports`. Dot-relative URLs can instead be prefixed by `options.base`.
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
                if let Some(transform) =
                    transform_srcset_with_base(&value, base, options, enable_imports, imports)
                {
                    match transform {
                        SrcsetTransform::Static(rewritten) => {
                            attr.value = Some(rewritten);
                        }
                        SrcsetTransform::Expression(expression) => {
                            *prop = asset_bind_directive(
                                "srcset",
                                expression,
                                attr.span,
                                attr.name_span,
                                value_span,
                            );
                        }
                    }
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

/// Returns asset attribute markers present on a template element.
///
/// The result uses the `asset:<attribute>` marker form consumed by higher-level
/// summaries and conformance probes.
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

enum SrcsetTransform {
    Static(String),
    Expression(String),
}

fn transform_srcset_with_base(
    value: &str,
    base: &str,
    options: &AssetUrlOptions,
    enable_imports: bool,
    imports: &mut Vec<Vue3ImportItem>,
) -> Option<SrcsetTransform> {
    let mut candidates = parse_srcset_candidates(value);
    if !candidates
        .iter()
        .any(|(url, _)| should_process_asset_url(url, options))
    {
        return None;
    }

    let mut rewritten_items = Vec::<String>::new();
    let mut need_import_transform = false;
    for (url, descriptor) in &mut candidates {
        if url.starts_with('.') {
            *url = join_asset_base(base, url);
            if descriptor.is_empty() {
                rewritten_items.push(url.clone());
            } else {
                rewritten_items.push(format!("{url} {descriptor}"));
            }
        } else if should_process_asset_url(url, options) {
            need_import_transform = true;
        } else if descriptor.is_empty() {
            rewritten_items.push(url.clone());
        } else {
            rewritten_items.push(format!("{url} {descriptor}"));
        }
    }

    if !need_import_transform {
        return Some(SrcsetTransform::Static(rewritten_items.join(", ")));
    }

    if enable_imports {
        asset_srcset_import_expression_from_candidates(&candidates, options, imports)
            .map(SrcsetTransform::Expression)
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
    asset_srcset_import_expression_from_candidates(&candidates, options, imports)
}

fn asset_srcset_import_expression_from_candidates(
    candidates: &[(String, String)],
    options: &AssetUrlOptions,
    imports: &mut Vec<Vue3ImportItem>,
) -> Option<String> {
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

/// Returns whether a URL is relative under Vue 3 SFC asset transform rules.
pub fn is_relative_url(url: &str) -> bool {
    matches!(url.chars().next(), Some('.' | '~' | '@' | '#'))
}

/// Returns whether a URL is an external HTTP(S) or protocol-relative URL.
pub fn is_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

/// Returns whether a URL is a data URL after trimming leading whitespace.
pub fn is_data_url(url: &str) -> bool {
    url.trim_start().to_ascii_lowercase().starts_with("data:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vuec_ast::Vue3Attribute;

    fn attr(name: &str, value: &str) -> Vue3Prop {
        Vue3Prop::Attribute(Vue3Attribute {
            name: name.into(),
            value: Some(value.into()),
            span: None,
            name_span: None,
            value_span: None,
            quote: None,
        })
    }

    fn directive_expression(prop: &Vue3Prop) -> &str {
        let Vue3Prop::Directive(directive) = prop else {
            panic!("expected directive prop, got {prop:?}");
        };
        let Some(Vue3Expression::Raw(expression)) = directive.exp.as_ref() else {
            panic!("expected raw directive expression, got {directive:?}");
        };
        expression
    }

    fn attribute_value(prop: &Vue3Prop) -> &str {
        let Vue3Prop::Attribute(attribute) = prop else {
            panic!("expected attribute prop, got {prop:?}");
        };
        attribute.value.as_deref().expect("attribute value")
    }

    #[test]
    fn template_utils_public_url_predicates_match_vue3_sfc_rules() {
        assert!(is_relative_url("./logo.png"));
        assert!(is_relative_url("~/logo.png"));
        assert!(is_relative_url("@/logo.png"));
        assert!(is_relative_url("#src/assets/logo.svg"));
        assert!(!is_relative_url("/logo.png"));
        assert!(!is_relative_url("logo.png"));

        assert!(is_external_url("http://vuejs.org/"));
        assert!(is_external_url("https://vuejs.org/"));
        assert!(is_external_url("//vuejs.org/"));
        assert!(!is_external_url("/vuejs.org/"));

        assert!(is_data_url("data:,i"));
        assert!(is_data_url("data:image/png;base64,i"));
        assert!(is_data_url("  DATA:image/png,i"));
        assert!(!is_data_url("./data:image/png,i"));
    }

    #[test]
    fn explicit_base_rewrites_only_dot_relative_asset_urls() {
        let mut props = vec![
            attr("src", "./bar.png"),
            attr("poster", "~poster.png"),
            attr("data-id", "./ignored.png"),
        ];
        let mut imports = Vec::new();

        transform_asset_url_props(
            "video",
            &mut props,
            &AssetUrlOptions {
                base: Some("/foo".into()),
                ..AssetUrlOptions::default()
            },
            true,
            &mut imports,
        );

        assert_eq!(attribute_value(&props[0]), "/foo/bar.png");
        assert_eq!(directive_expression(&props[1]), "_imports_0");
        assert_eq!(imports[0].path, "poster.png");
        assert_eq!(attribute_value(&props[2]), "./ignored.png");
    }

    #[test]
    fn custom_asset_url_tags_import_tilde_with_explicit_base() {
        let mut props = vec![attr("bar", "~baz")];
        let mut imports = Vec::new();
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);

        transform_asset_url_props(
            "foo",
            &mut props,
            &AssetUrlOptions {
                base: Some("/foo".into()),
                tags,
                ..AssetUrlOptions::default()
            },
            true,
            &mut imports,
        );

        assert_eq!(directive_expression(&props[0]), "_imports_0");
        assert_eq!(imports[0].path, "baz");
    }

    #[test]
    fn explicit_base_srcset_rewrites_dot_candidates_and_imports_alias_candidates() {
        let mut props = vec![attr("srcset", "@/logo.png 1x, ./logo.png 2x")];
        let mut imports = Vec::new();

        transform_asset_url_props(
            "img",
            &mut props,
            &AssetUrlOptions {
                base: Some("/foo/".into()),
                ..AssetUrlOptions::default()
            },
            true,
            &mut imports,
        );

        assert_eq!(
            directive_expression(&props[0]),
            r#"_imports_0 + ' 1x, ' + "/foo/logo.png" + ' 2x'"#
        );
        assert_eq!(imports[0].path, "@/logo.png");
    }

    #[test]
    fn srcset_imports_subpath_hash_and_preserves_svg_fragments() {
        let mut props = vec![attr(
            "srcset",
            "#src/assets/vue.svg, ./icons.svg#icon-heart 2x, ./foo%.png 3x",
        )];
        let mut imports = Vec::new();

        transform_asset_url_props(
            "img",
            &mut props,
            &AssetUrlOptions::default(),
            true,
            &mut imports,
        );

        assert_eq!(
            directive_expression(&props[0]),
            "_imports_0 + ', ' + _imports_1 + '#icon-heart' + ' 2x, ' + _imports_2 + ' 3x'"
        );
        assert_eq!(imports[0].path, "#src/assets/vue.svg");
        assert_eq!(imports[1].path, "./icons.svg");
        assert_eq!(imports[2].path, "./foo%.png");
    }

    #[test]
    fn pure_hash_values_on_custom_tags_stay_static() {
        let mut props = vec![attr("bar", "#src/assets/vue.svg")];
        let mut imports = Vec::new();
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);

        transform_asset_url_props(
            "foo",
            &mut props,
            &AssetUrlOptions {
                include_absolute: true,
                tags,
                ..AssetUrlOptions::default()
            },
            true,
            &mut imports,
        );

        assert_eq!(attribute_value(&props[0]), "#src/assets/vue.svg");
        assert!(imports.is_empty());
    }
}
