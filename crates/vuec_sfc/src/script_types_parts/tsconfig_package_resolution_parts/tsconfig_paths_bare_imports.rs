pub(crate) fn vue3_tsconfig_direct_path_mappings(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
) -> Vec<Vue3TsconfigPathMapping> {
    let Some(compiler_options) = value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let target_base_dir = compiler_options
        .get("baseUrl")
        .and_then(serde_json::Value::as_str)
        .map(|base_url| vue3_tsconfig_target_path(config_dir, template_config_dir, base_url, ""))
        .unwrap_or_else(|| config_dir.to_path_buf());
    let Some(paths) = compiler_options
        .get("paths")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    paths
        .iter()
        .filter_map(|(pattern, targets)| {
            let targets = vue3_tsconfig_path_target_values(targets);
            (!targets.is_empty()).then(|| Vue3TsconfigPathMapping {
                pattern: pattern.clone(),
                targets,
                target_base_dir: target_base_dir.clone(),
                template_config_dir: template_config_dir.to_path_buf(),
            })
        })
        .collect()
}

pub(crate) fn vue3_tsconfig_path_target_values(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(targets) => targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
        serde_json::Value::String(target) => vec![target.to_string()],
        _ => Vec::new(),
    }
}

pub(crate) fn resolve_vue3_tsconfig_path_mappings(
    mappings: &[Vue3TsconfigPathMapping],
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let mut matches = mappings
        .iter()
        .enumerate()
        .filter_map(|(order, mapping)| {
            vue3_tsconfig_path_pattern_capture(&mapping.pattern, source).map(|(score, capture)| {
                Vue3TsconfigPathMatch {
                    mapping,
                    capture,
                    score,
                    order,
                }
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.order.cmp(&right.order))
    });
    for matched in matches {
        for target in &matched.mapping.targets {
            let candidate = vue3_tsconfig_target_path(
                &matched.mapping.target_base_dir,
                &matched.mapping.template_config_dir,
                target,
                &matched.capture,
            );
            if let Some(resolved) = resolve_vue3_type_import_path(&candidate, type_resolver) {
                return Some(resolved);
            }
        }
    }
    None
}

pub(crate) fn vue3_tsconfig_path_pattern_capture(
    pattern: &str,
    source: &str,
) -> Option<(usize, String)> {
    let Some(star) = pattern.find('*') else {
        return (pattern == source).then(|| (usize::MAX, String::new()));
    };
    if pattern[star + 1..].contains('*') {
        return None;
    }
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !source.starts_with(prefix)
        || !source.ends_with(suffix)
        || source.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some((
        prefix.len() + suffix.len(),
        source[prefix.len()..source.len() - suffix.len()].to_string(),
    ))
}

pub(crate) fn vue3_tsconfig_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    capture: &str,
) -> PathBuf {
    let target = target.replace('*', capture);
    let target = target.replace(
        "${configDir}",
        normalize_path_string(template_config_dir).as_str(),
    );
    let path = Path::new(&target);
    if path.is_absolute() {
        normalize_path_components(PathBuf::from(target))
    } else {
        normalize_path_components(target_base_dir.join(target))
    }
}

pub(crate) fn resolve_vue3_bare_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let (package_name, subpath) = vue3_package_import_parts(source)?;
    for node_modules in vue3_node_modules_search_paths(filename) {
        let package_dir = node_modules.join(&package_name);
        if package_dir.is_dir() {
            if let Some(resolved) =
                resolve_vue3_package_type_entry(&package_dir, subpath.as_deref(), type_resolver)
            {
                return Some(resolved);
            }
        }
        let types_package_dir = node_modules.join(vue3_at_types_package_name(&package_name));
        if types_package_dir.is_dir() {
            if let Some(resolved) = resolve_vue3_package_type_entry(
                &types_package_dir,
                subpath.as_deref(),
                type_resolver,
            ) {
                return Some(resolved);
            }
        }
    }
    None
}

pub(crate) fn vue3_package_import_parts(source: &str) -> Option<(String, Option<String>)> {
    if source.is_empty()
        || source.starts_with('.')
        || source.starts_with('/')
        || source.starts_with('#')
        || source.contains(':')
    {
        return None;
    }
    let parts = source.split('/').collect::<Vec<_>>();
    if parts.first().is_some_and(|part| part.starts_with('@')) {
        if parts.len() < 2 || parts[0].len() <= 1 || parts[1].is_empty() {
            return None;
        }
        let package_name = format!("{}/{}", parts[0], parts[1]);
        let subpath = (parts.len() > 2).then(|| parts[2..].join("/"));
        return Some((package_name, subpath));
    }
    let package_name = parts.first().filter(|part| !part.is_empty())?.to_string();
    let subpath = (parts.len() > 1).then(|| parts[1..].join("/"));
    Some((package_name, subpath))
}

pub(crate) fn vue3_node_modules_search_paths(filename: &str) -> Vec<PathBuf> {
    let Some(current) = Path::new(filename).parent() else {
        return Vec::new();
    };
    vue3_node_modules_search_paths_from_dir(current)
}

pub(crate) fn vue3_node_modules_search_paths_from_dir(start_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        paths.push(normalize_path_components(dir.join("node_modules")));
        current = dir.parent();
    }
    paths
}

pub(crate) fn vue3_type_resolver_context_for_filename(filename: &str) -> Vue3TypeResolverContext {
    Vue3TypeResolverContext {
        typescript_version: vue3_typescript_version_for_filename(filename)
            .unwrap_or_else(vue3_package_typescript_baseline_version),
    }
}

pub(crate) fn vue3_typescript_version_for_filename(
    filename: &str,
) -> Option<nodejs_semver::Version> {
    vue3_node_modules_search_paths(filename)
        .into_iter()
        .find_map(|node_modules| {
            vue3_typescript_version_from_package_json(
                &node_modules.join("typescript").join("package.json"),
            )
        })
}

pub(crate) fn vue3_typescript_version_from_package_json(
    package_json: &Path,
) -> Option<nodejs_semver::Version> {
    let source = std::fs::read_to_string(package_json).ok()?;
    let package = serde_json::from_str::<serde_json::Value>(&source).ok()?;
    let version = package.get("version")?.as_str()?.trim();
    nodejs_semver::Version::parse(version).ok()
}

pub(crate) fn vue3_at_types_package_name(package_name: &str) -> PathBuf {
    if let Some(scoped) = package_name.strip_prefix('@') {
        return PathBuf::from("@types").join(scoped.replace('/', "__"));
    }
    PathBuf::from("@types").join(package_name)
}

pub(crate) fn resolve_vue3_package_type_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    match resolve_vue3_package_json_type_entry(package_dir, subpath, type_resolver) {
        Vue3PackageJsonTypeResolution::Resolved(path) => return Some(path),
        Vue3PackageJsonTypeResolution::Blocked => return None,
        Vue3PackageJsonTypeResolution::NoPackageJson
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {}
    }
    let candidate = subpath
        .map(|subpath| package_dir.join(subpath))
        .unwrap_or_else(|| package_dir.to_path_buf());
    resolve_vue3_type_import_path(&candidate, type_resolver)
}
