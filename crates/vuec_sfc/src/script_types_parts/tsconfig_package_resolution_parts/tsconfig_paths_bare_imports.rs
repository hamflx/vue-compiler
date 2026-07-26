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
    let configured_base_url = if type_resolver.typescript_version < (7, 0, 0).into() {
        compiler_options
            .get("baseUrl")
            .and_then(serde_json::Value::as_str)
    } else {
        None
    };
    let target_base_dir = if let Some(base_url) = configured_base_url {
        let Some(path) = vue3_tsconfig_target_path(
            config_dir,
            template_config_dir,
            base_url,
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
        .map(|(pattern, targets)| Vue3TsconfigPathMapping {
            pattern: pattern.clone(),
            targets: vue3_tsconfig_path_target_values(targets),
            target_base_dir: target_base_dir.clone(),
            template_config_dir: template_config_dir.to_path_buf(),
        })
        .collect()
}

fn vue3_tsconfig_declares_compiler_option(value: &serde_json::Value, option: &str) -> bool {
    value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|compiler_options| compiler_options.contains_key(option))
}

fn vue3_tsconfig_direct_base_url(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let base_url = value
        .get("compilerOptions")?
        .get("baseUrl")?
        .as_str()?;
    vue3_tsconfig_target_path(
        config_dir,
        template_config_dir,
        base_url,
        type_resolver,
    )
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

#[cfg(test)]
pub(crate) fn resolve_vue3_tsconfig_path_mappings(
    mappings: &[Vue3TsconfigPathMapping],
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_tsconfig_path_mappings_with_mode(
        mappings,
        source,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_tsconfig_path_mappings_with_mode(
    mappings: &[Vue3TsconfigPathMapping],
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let (mapping_index, capture) = vue3_typescript_best_path_pattern_match(
        mappings
            .iter()
            .enumerate()
            .map(|(index, mapping)| (index, mapping.pattern.as_str())),
        source,
    )?;
    let mapping = &mappings[mapping_index];
    for target in &mapping.targets {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        let candidate = vue3_tsconfig_path_mapping_target_path(
            &mapping.target_base_dir,
            &mapping.template_config_dir,
            target,
            &capture,
            type_resolver,
        )?;
        let resolved = resolve_vue3_metadata_type_import_path_with_mode(
            &candidate,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if let Some(resolved) = resolved {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn resolve_vue3_tsconfig_base_url_with_mode(
    base_url: &Path,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if source.is_empty() || vue3_type_import_source_is_relative(source) {
        return None;
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(source)
    {
        return None;
    }
    let candidate = normalize_path_components(base_url.join(source));
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&candidate))
    {
        return None;
    }
    resolve_vue3_metadata_type_import_path_with_mode(
        &candidate,
        resolution_mode,
        type_resolver,
    )
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
        prefix.len(),
        source[prefix.len()..source.len() - suffix.len()].to_string(),
    ))
}

fn vue3_typescript_best_path_pattern_match<'a>(
    patterns: impl IntoIterator<Item = (usize, &'a str)>,
    source: &str,
) -> Option<(usize, String)> {
    patterns
        .into_iter()
        .filter_map(|(index, pattern)| {
            vue3_tsconfig_path_pattern_capture(pattern, source)
                .map(|(prefix_len, capture)| (prefix_len, index, capture))
        })
        .fold(
            None,
            |best: Option<(usize, usize, String)>, candidate| match best {
                Some(best) if best.0 >= candidate.0 => Some(best),
                _ => Some(candidate),
            },
        )
        .map(|(_, index, capture)| (index, capture))
}

pub(crate) fn vue3_tsconfig_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target = vue3_tsconfig_expand_config_dir_template(
        target,
        template_config_dir,
        type_resolver,
    )?;
    Some(vue3_tsconfig_path_from_expanded_target(
        target_base_dir,
        &target,
    ))
}

pub(crate) fn vue3_tsconfig_path_mapping_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    capture: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target =
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, type_resolver)?;
    let target =
        vue3_typescript_path_target_substitution(&target, capture, type_resolver)?;
    Some(vue3_tsconfig_path_from_expanded_target(
        target_base_dir,
        &target,
    ))
}

fn vue3_typescript_path_target_substitution(
    target: &str,
    capture: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    if capture.is_empty() {
        type_resolver
            .external_type_session
            .concat_metadata_path("", target)
    } else {
        type_resolver
            .external_type_session
            .replace_first_metadata_path_pattern(target, "*", capture)
    }
}

fn vue3_tsconfig_path_from_expanded_target(target_base_dir: &Path, target: &str) -> PathBuf {
    let path = Path::new(target);
    if path.is_absolute() {
        normalize_path_components(PathBuf::from(target))
    } else {
        normalize_path_components(target_base_dir.join(target))
    }
}

pub(crate) fn vue3_tsconfig_expand_config_dir_template(
    target: &str,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    const CONFIG_DIR_TEMPLATE: &str = "${configDir}";

    let Some(suffix) = target.strip_prefix(CONFIG_DIR_TEMPLATE) else {
        return type_resolver
            .external_type_session
            .concat_metadata_path("", target);
    };
    let template_config_dir = normalize_path_string(template_config_dir);
    type_resolver
        .external_type_session
        .concat_metadata_path(&template_config_dir, suffix)
}

#[cfg(test)]
pub(crate) fn resolve_vue3_bare_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_bare_type_import_with_mode(
        filename,
        source,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_bare_type_import_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let (package_name, subpath) = vue3_package_import_parts(source)?;
    for node_modules in vue3_node_modules_search_paths(filename, type_resolver) {
        let package_dir = node_modules.join(&package_name);
        let is_package_dir = type_resolver
            .external_type_session
            .metadata_path_is_dir(&package_dir)?;
        if is_package_dir {
            let resolved = resolve_vue3_package_type_entry_with_mode(
                &package_dir,
                subpath.as_deref(),
                resolution_mode,
                type_resolver,
            );
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if let Some(resolved) = resolved {
                return Some(resolved);
            }
        }
        let types_package_dir = node_modules.join(vue3_at_types_package_name(&package_name));
        let is_types_package_dir = type_resolver
            .external_type_session
            .metadata_path_is_dir(&types_package_dir)?;
        if is_types_package_dir {
            let resolved = resolve_vue3_package_type_entry_with_mode(
                &types_package_dir,
                subpath.as_deref(),
                resolution_mode,
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
    stop_before_node_modules: bool,
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
            stop_before_node_modules: false,
            session,
        }
    }

    fn package_scope(mut self) -> Self {
        self.stop_before_node_modules = true;
        self
    }
}

impl Iterator for Vue3AncestorSearchPaths<'_> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        let dir = self.current.take()?;
        if self.stop_before_node_modules
            && dir
                .file_name()
                .is_some_and(vue3_path_component_is_node_modules)
        {
            return None;
        }
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

enum Vue3PackageScopeResolution {
    Found {
        package_dir: PathBuf,
        manifest: std::sync::Arc<Vue3PackageJsonTypeManifest>,
    },
    Missing,
    MetadataBlocked,
}

fn vue3_package_scope_for_path(
    path: &Path,
    session: &Vue3ExternalTypeLoadSession,
) -> Vue3PackageScopeResolution {
    let path = normalize_path_components(path.to_path_buf());
    for package_json in
        Vue3AncestorSearchPaths::new(path.parent(), "package.json", session).package_scope()
    {
        match session.package_json_from_path(&package_json) {
            Some(manifest) => {
                let package_dir = package_json
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_path_buf();
                return Vue3PackageScopeResolution::Found {
                    package_dir,
                    manifest,
                };
            }
            None if session.metadata_is_blocked() => {
                return Vue3PackageScopeResolution::MetadataBlocked;
            }
            None => {}
        }
    }
    if session.metadata_is_blocked() {
        Vue3PackageScopeResolution::MetadataBlocked
    } else {
        Vue3PackageScopeResolution::Missing
    }
}

fn vue3_package_module_type_for_path(
    path: &Path,
    session: &Vue3ExternalTypeLoadSession,
) -> Option<Vue3PackageModuleType> {
    match vue3_package_scope_for_path(path, session) {
        Vue3PackageScopeResolution::Found { manifest, .. } => Some(manifest.module_type),
        Vue3PackageScopeResolution::Missing => Some(Vue3PackageModuleType::CommonJs),
        Vue3PackageScopeResolution::MetadataBlocked => None,
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
    resolve_vue3_package_type_entry_with_mode(
        package_dir,
        subpath,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_package_type_entry_with_mode(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    match resolve_vue3_package_json_type_entry_with_mode(
        package_dir,
        subpath,
        resolution_mode,
        type_resolver,
    ) {
        Vue3PackageJsonTypeResolution::Resolved(path) => return Some(path),
        Vue3PackageJsonTypeResolution::Blocked => return None,
        Vue3PackageJsonTypeResolution::NoPackageJson
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {}
    }
    let candidate = subpath
        .map(|subpath| package_dir.join(subpath))
        .unwrap_or_else(|| package_dir.to_path_buf());
    resolve_vue3_metadata_type_import_path_with_mode(&candidate, resolution_mode, type_resolver)
}
