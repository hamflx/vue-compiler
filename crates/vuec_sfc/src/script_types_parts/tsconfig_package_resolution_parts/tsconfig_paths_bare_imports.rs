pub(crate) fn vue3_tsconfig_direct_path_mappings(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<Vue3TsconfigPathMapping> {
    let Some(compiler_options) = value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let target_base_dir = if let Some(base_url) = compiler_options
        .get("baseUrl")
        .and_then(serde_json::Value::as_str)
    {
        let Some(path) = vue3_tsconfig_target_path(
            config_dir,
            template_config_dir,
            base_url,
            "",
            type_resolver,
        ) else {
            return Vec::new();
        };
        path
    } else {
        config_dir.to_path_buf()
    };
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
                type_resolver,
            )?;
            let resolved = resolve_vue3_type_import_path(&candidate, type_resolver);
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if let Some(resolved) = resolved {
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
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target = type_resolver.external_type_session.replace_metadata_path_pattern(
        target,
        "*",
        capture,
    )?;
    let template_config_dir = normalize_path_string(template_config_dir);
    let target = type_resolver.external_type_session.replace_metadata_path_pattern(
        &target,
        "${configDir}",
        &template_config_dir,
    )?;
    let path = Path::new(&target);
    if path.is_absolute() {
        Some(normalize_path_components(PathBuf::from(target)))
    } else {
        Some(normalize_path_components(target_base_dir.join(target)))
    }
}

pub(crate) fn resolve_vue3_bare_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let (package_name, subpath) = vue3_package_import_parts(source)?;
    for node_modules in vue3_node_modules_search_paths(filename, type_resolver) {
        let package_dir = node_modules.join(&package_name);
        if package_dir.is_dir() {
            let resolved =
                resolve_vue3_package_type_entry(&package_dir, subpath.as_deref(), type_resolver);
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if let Some(resolved) = resolved {
                return Some(resolved);
            }
        }
        let types_package_dir = node_modules.join(vue3_at_types_package_name(&package_name));
        if types_package_dir.is_dir() {
            let resolved = resolve_vue3_package_type_entry(
                &types_package_dir,
                subpath.as_deref(),
                type_resolver,
            );
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if let Some(resolved) = resolved {
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
        || source.contains('\\')
    {
        return None;
    }
    let parts = source.split('/').collect::<Vec<_>>();
    // Bare package fallback must not normalize outside the selected package root.
    if parts
        .iter()
        .any(|part| part.is_empty() || matches!(*part, "." | ".."))
    {
        return None;
    }
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

pub(crate) fn vue3_ancestor_search_candidate_weight(dir: &Path, suffix: &str) -> usize {
    dir.as_os_str()
        .as_encoded_bytes()
        .len()
        .saturating_add(usize::from(!dir.as_os_str().is_empty()))
        .saturating_add(suffix.len())
}

struct Vue3AncestorSearchPaths<'a> {
    current: Option<&'a Path>,
    suffix: &'static str,
    remaining_depth: usize,
    session: &'a Vue3ExternalTypeLoadSession,
}

impl<'a> Vue3AncestorSearchPaths<'a> {
    fn new(
        current: Option<&'a Path>,
        suffix: &'static str,
        session: &'a Vue3ExternalTypeLoadSession,
    ) -> Self {
        Self {
            current,
            suffix,
            remaining_depth: session.max_ancestor_search_depth(),
            session,
        }
    }
}

impl Iterator for Vue3AncestorSearchPaths<'_> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        let dir = self.current.take()?;
        self.current = dir.parent();
        if self.remaining_depth == 0 {
            self.current = None;
            self.session.block_metadata();
            return None;
        }
        self.remaining_depth -= 1;
        if !self.session.claim_ancestor_search_dir(dir, self.suffix) {
            self.current = None;
            return None;
        }
        Some(normalize_path_components(dir.join(self.suffix)))
    }
}

pub(crate) fn vue3_node_modules_search_paths<'a>(
    filename: &'a str,
    type_resolver: &'a Vue3TypeResolverContext,
) -> impl Iterator<Item = PathBuf> + 'a {
    Vue3AncestorSearchPaths::new(
        Path::new(filename).parent(),
        "node_modules",
        &type_resolver.external_type_session,
    )
}

pub(crate) fn vue3_node_modules_search_paths_from_dir<'a>(
    start_dir: &'a Path,
    type_resolver: &'a Vue3TypeResolverContext,
) -> impl Iterator<Item = PathBuf> + 'a {
    Vue3AncestorSearchPaths::new(
        Some(start_dir),
        "node_modules",
        &type_resolver.external_type_session,
    )
}

pub(crate) fn vue3_type_resolver_context_for_filename(filename: &str) -> Vue3TypeResolverContext {
    let mut type_resolver = Vue3TypeResolverContext::default();
    if let Some(version) = vue3_typescript_version_for_filename(filename, &type_resolver) {
        type_resolver.typescript_version = version;
    }
    type_resolver
}

pub(crate) fn vue3_typescript_version_for_filename(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<nodejs_semver::Version> {
    vue3_node_modules_search_paths(filename, type_resolver).find_map(|node_modules| {
        vue3_typescript_version_from_package_json(
            &node_modules.join("typescript").join("package.json"),
            type_resolver,
        )
    })
}

pub(crate) fn vue3_typescript_version_from_package_json(
    package_json: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<nodejs_semver::Version> {
    let package = type_resolver
        .external_type_session
        .package_json_from_path(package_json)?;
    let version = package
        .version
        .as_ref()
        .and_then(serde_json::Value::as_str)?
        .trim();
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
