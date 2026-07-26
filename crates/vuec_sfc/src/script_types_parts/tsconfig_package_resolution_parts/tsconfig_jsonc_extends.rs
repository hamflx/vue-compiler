pub(crate) fn vue3_parse_tsconfig_jsonc(source: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(source)
        .ok()
        .or_else(|| {
            let normalized = vue3_normalize_tsconfig_jsonc(source);
            serde_json::from_str::<serde_json::Value>(&normalized).ok()
        })
}

pub(crate) fn vue3_normalize_tsconfig_jsonc(source: &str) -> String {
    let without_comments = vue3_strip_jsonc_comments(source);
    vue3_strip_jsonc_trailing_commas(&without_comments)
}

pub(crate) fn vue3_strip_jsonc_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch != '/' {
            output.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('/') => {
                chars.next();
                output.push(' ');
                output.push(' ');
                for comment in chars.by_ref() {
                    if comment == '\n' || comment == '\r' {
                        output.push(comment);
                        break;
                    }
                    output.push(' ');
                }
            }
            Some('*') => {
                chars.next();
                output.push(' ');
                output.push(' ');
                let mut prev_star = false;
                for comment in chars.by_ref() {
                    let ends_comment = prev_star && comment == '/';
                    if comment == '\n' || comment == '\r' {
                        output.push(comment);
                    } else {
                        output.push(' ');
                    }
                    if ends_comment {
                        break;
                    }
                    prev_star = comment == '*';
                }
            }
            _ => output.push(ch),
        }
    }
    output
}

pub(crate) fn vue3_strip_jsonc_trailing_commas(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch != ',' {
            output.push(ch);
            continue;
        }
        let mut lookahead = chars.clone();
        while lookahead.peek().is_some_and(|next| next.is_whitespace()) {
            lookahead.next();
        }
        if lookahead
            .peek()
            .is_some_and(|next| matches!(*next, '}' | ']'))
        {
            continue;
        }
        output.push(ch);
    }
    output
}

pub(crate) fn vue3_tsconfig_extends_paths(
    value: &serde_json::Value,
    config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    match value.get("extends") {
        Some(serde_json::Value::String(target)) => {
            if !type_resolver
                .external_type_session
                .claim_metadata_fanout_entry()
            {
                return Vec::new();
            }
            vue3_resolve_tsconfig_extends_path(config_dir, target, type_resolver)
                .into_iter()
                .collect()
        }
        Some(serde_json::Value::Array(targets)) => {
            let mut paths = Vec::new();
            for target in targets.iter().filter_map(serde_json::Value::as_str) {
                if !type_resolver
                    .external_type_session
                    .claim_metadata_fanout_entry()
                {
                    return Vec::new();
                }
                if let Some(path) =
                    vue3_resolve_tsconfig_extends_path(config_dir, target, type_resolver)
                {
                    paths.push(path);
                }
                if type_resolver.external_type_session.metadata_is_blocked() {
                    return Vec::new();
                }
            }
            paths
        }
        _ => Vec::new(),
    }
}

pub(crate) fn vue3_tsconfig_reference_paths(
    value: &serde_json::Value,
    config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let targets = value
        .get("references")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reference| reference.get("path").and_then(serde_json::Value::as_str));
    let mut paths = Vec::new();
    for target in targets {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return Vec::new();
        }
        if let Some(path) =
            vue3_resolve_tsconfig_reference_path(config_dir, target, type_resolver)
        {
            paths.push(path);
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    paths
}

pub(crate) fn vue3_resolve_tsconfig_extends_path(
    config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target = vue3_normalize_typescript_path_separators(target, type_resolver)?;
    if vue3_tsconfig_path_is_relative(&target) || Path::new(&target).is_absolute() {
        return vue3_resolve_normalized_tsconfig_path(config_dir, &target, type_resolver);
    }
    resolve_vue3_package_tsconfig_extends(config_dir, &target, type_resolver)
}

pub(crate) fn vue3_resolve_tsconfig_reference_path(
    config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target = vue3_normalize_typescript_path_separators(target, type_resolver)?;
    vue3_resolve_normalized_tsconfig_path(config_dir, &target, type_resolver)
}

fn vue3_resolve_normalized_tsconfig_path(
    config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let candidate = if Path::new(target).is_absolute() {
        normalize_path_components(PathBuf::from(target))
    } else {
        normalize_path_components(config_dir.join(target))
    };
    resolve_vue3_tsconfig_candidate_path(&candidate, false, type_resolver)
}

pub(crate) fn vue3_tsconfig_path_is_relative(target: &str) -> bool {
    target.starts_with("./")
        || target.starts_with("../")
        || target.starts_with(".\\")
        || target.starts_with("..\\")
}

pub(crate) fn vue3_normalize_typescript_path_separators(
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    type_resolver
        .external_type_session
        .replace_metadata_path_pattern(target, "\\", "/")
}

pub(crate) fn resolve_vue3_tsconfig_candidate_path(
    candidate: &Path,
    include_index: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(candidate))
    {
        return None;
    }
    let mut candidates = if candidate.extension().is_some() {
        vec![candidate.to_path_buf()]
    } else {
        vec![
            path_with_extension(candidate, "json"),
            candidate.join("tsconfig.json"),
        ]
    };
    if include_index && candidate.extension().is_none() {
        candidates.push(candidate.join("index.json"));
    }
    for path in candidates {
        if !type_resolver
            .external_type_session
            .metadata_path_is_within_limit(&normalize_path_string(&path))
        {
            return None;
        }
        if type_resolver
            .external_type_session
            .metadata_path_is_file(&path)?
        {
            return Some(path);
        }
    }
    None
}

pub(crate) fn resolve_vue3_package_tsconfig_extends(
    config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let (package_name, subpath) = vue3_package_import_parts(target)?;
    for node_modules in vue3_node_modules_search_paths_from_dir(config_dir, type_resolver) {
        let package_dir = normalize_path_components(node_modules.join(&package_name));
        if !type_resolver
            .external_type_session
            .metadata_path_is_dir(&package_dir)?
        {
            continue;
        }
        let resolved =
            resolve_vue3_package_tsconfig_entry(&package_dir, subpath.as_deref(), type_resolver);
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if let Some(resolved) = resolved {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn resolve_vue3_package_tsconfig_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if let Some(subpath) = subpath {
        return vue3_package_tsconfig_subpath(package_dir, subpath, type_resolver);
    }
    let manifest_entry = vue3_package_json_tsconfig_entry(package_dir, type_resolver);
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if let Some(manifest_entry) = manifest_entry {
        return Some(manifest_entry);
    }
    if let Some(tsconfig) = resolve_vue3_tsconfig_candidate_path(
        &package_dir.join("tsconfig"),
        false,
        type_resolver,
    ) {
        return Some(tsconfig);
    }
    let index = package_dir.join("index.json");
    type_resolver
        .external_type_session
        .metadata_path_is_file(&index)?
        .then_some(index)
}

pub(crate) fn vue3_package_tsconfig_subpath(
    package_dir: &Path,
    subpath: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !vue3_package_tsconfig_subpath_is_safe(subpath) {
        return None;
    }
    let candidate = normalize_path_components(package_dir.join(subpath));
    resolve_vue3_tsconfig_candidate_path(&candidate, true, type_resolver)
}

pub(crate) fn vue3_package_json_tsconfig_entry(
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let package_json = package_dir.join("package.json");
    let manifest = type_resolver
        .external_type_session
        .package_json_from_path(&package_json)?;
    let target = manifest
        .tsconfig
        .as_ref()
        .and_then(serde_json::Value::as_str)?;
    let target = vue3_normalize_typescript_path_separators(target, type_resolver)?;
    if !vue3_package_tsconfig_target_is_safe(&target) {
        return None;
    }
    let target = target.trim_start_matches("./");
    let candidate = normalize_path_components(package_dir.join(target));
    resolve_vue3_tsconfig_candidate_path(&candidate, true, type_resolver)
}

pub(crate) fn vue3_package_tsconfig_subpath_is_safe(subpath: &str) -> bool {
    !subpath.is_empty()
        && !subpath.contains(':')
        && !subpath
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && Path::new(subpath).components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

pub(crate) fn vue3_package_tsconfig_target_is_safe(target: &str) -> bool {
    !target.is_empty()
        && !target.contains(':')
        && !Path::new(target).is_absolute()
        && Path::new(target).components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}
