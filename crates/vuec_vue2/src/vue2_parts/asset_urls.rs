fn process_sfc_asset_url_transform(element: &mut Vue2Element, options: &Vue2CompileOptions) {
    let Some(transform) = options.sfc_asset_url_transform.as_ref() else {
        return;
    };
    let asset_attrs = vue2_sfc_asset_attrs_for_tag(&element.tag, transform);
    let has_srcset_transform = matches!(element.tag.as_str(), "img" | "source");
    if asset_attrs.is_empty() && !has_srcset_transform {
        return;
    }
    for attr in &mut element.attrs {
        let should_rewrite_asset = asset_attrs.iter().any(|candidate| candidate == &attr.name);
        if should_rewrite_asset {
            if let Some(raw) = static_attr_raw_value(&attr.value) {
                attr.value = vue2_sfc_url_to_require(&raw, transform);
            }
        }
        if has_srcset_transform && attr.name == "srcset" {
            if let Some(raw) = static_attr_raw_value(&attr.value) {
                attr.value = vue2_sfc_srcset_to_require(&raw, transform);
            }
        }
    }
}

fn vue2_sfc_asset_attrs_for_tag(
    tag: &str,
    options: &Vue2SfcAssetUrlTransformOptions,
) -> Vec<String> {
    let mut attrs = options.tags.get(tag).cloned().unwrap_or_default();
    if let Some(wildcard) = options.tags.get("*") {
        attrs.extend(wildcard.iter().cloned());
    }
    attrs
}

fn static_attr_raw_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return None;
    }
    serde_json::from_str::<String>(value).ok()
}

fn vue2_sfc_srcset_to_require(value: &str, options: &Vue2SfcAssetUrlTransformOptions) -> String {
    let candidates = parse_vue2_sfc_srcset_candidates(value);
    if candidates.is_empty() {
        return js_string(value);
    }
    let mut code = String::new();
    for (url, descriptor) in candidates {
        code.push_str(&vue2_sfc_url_to_require(&url, options));
        code.push_str(" + ");
        code.push_str(&js_string(&format!(
            "{}{}, ",
            if descriptor.is_empty() { "" } else { " " },
            descriptor
        )));
        code.push_str(" + ");
    }
    code.truncate(code.len().saturating_sub(6));
    code.push('"');
    if code.ends_with(" + \"\"") {
        code.truncate(code.len() - " + \"\"".len());
    }
    code
}

fn parse_vue2_sfc_srcset_candidates(value: &str) -> Vec<(String, String)> {
    value
        .split(',')
        .filter_map(|candidate| {
            let normalized = candidate
                .replace(['\t', '\n', '\u{000C}', '\r'], " ")
                .trim()
                .to_string();
            if normalized.is_empty() {
                return None;
            }
            let (url, descriptor) = normalized.split_once(' ').map_or_else(
                || (normalized.clone(), String::new()),
                |(url, descriptor)| (url.to_string(), descriptor.trim().to_string()),
            );
            Some((url, descriptor))
        })
        .collect()
}

fn vue2_sfc_url_to_require(url: &str, options: &Vue2SfcAssetUrlTransformOptions) -> String {
    let first_char = url.chars().next();
    let mut normalized = url.to_string();
    if first_char == Some('~') {
        normalized = if url.chars().nth(1) == Some('/') {
            url.chars().skip(2).collect()
        } else {
            url.chars().skip(1).collect()
        };
    }

    if is_vue2_sfc_external_url(&normalized)
        || is_vue2_sfc_data_url(&normalized)
        || first_char == Some('#')
    {
        return js_string(url);
    }

    let (path, hash) = vue2_sfc_parse_url_parts(&normalized);
    if let Some(base) = options.base.as_deref().filter(|base| !base.is_empty()) {
        if first_char == Some('.') || first_char == Some('~') {
            return js_string(&vue2_sfc_join_base(base, &path, &hash));
        }
    }

    if options.include_absolute || matches!(first_char, Some('.' | '~' | '@')) {
        if hash.is_empty() {
            format!("require({})", js_string(&normalized))
        } else {
            format!("require({}) + {}", js_string(&path), js_string(&hash))
        }
    } else {
        js_string(url)
    }
}

fn vue2_sfc_parse_url_parts(url: &str) -> (String, String) {
    if url.is_empty() {
        return (String::new(), String::new());
    }
    if let Some(hash) = url.find('#') {
        (url[..hash].to_string(), url[hash..].to_string())
    } else {
        (url.to_string(), String::new())
    }
}

fn vue2_sfc_join_base(base: &str, path: &str, hash: &str) -> String {
    let (host, base_path) = split_vue2_sfc_base(base);
    let path = strip_vue2_sfc_leading_dot_segments(path);
    let mut joined = join_vue2_sfc_paths(base_path, &path);
    if joined.is_empty() {
        joined.push('/');
    }
    format!("{host}{joined}{hash}")
}

fn split_vue2_sfc_base(base: &str) -> (&str, &str) {
    if let Some(protocol) = base.find("://") {
        let after_protocol = protocol + 3;
        let rest = &base[after_protocol..];
        if let Some(slash) = rest.find('/') {
            let split = after_protocol + slash;
            return (&base[..split], &base[split..]);
        }
        return (base, "/");
    }
    if let Some(rest) = base.strip_prefix("//") {
        if let Some(slash) = rest.find('/') {
            let split = 2 + slash;
            return (&base[..split], &base[split..]);
        }
        return (base, "/");
    }
    ("", base)
}

fn strip_vue2_sfc_leading_dot_segments(path: &str) -> String {
    let mut rest = path;
    while let Some(stripped) = rest.strip_prefix("./") {
        rest = stripped;
    }
    rest.to_string()
}

fn join_vue2_sfc_paths(base: &str, path: &str) -> String {
    let absolute = base.starts_with('/');
    let mut parts = Vec::<&str>::new();
    for part in base.split('/').chain(path.split('/')) {
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
    joined
}

fn is_vue2_sfc_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

fn is_vue2_sfc_data_url(url: &str) -> bool {
    url.trim_start().to_ascii_lowercase().starts_with("data:")
}
