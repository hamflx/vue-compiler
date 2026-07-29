const VUE3_TSCONFIG_CONFIG_DIR_TEMPLATE: &str = "${configDir}";

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
        if !type_resolver
            .external_type_session
            .claim_tsconfig_materialization(
                std::mem::size_of::<PathBuf>()
                    .saturating_add(config_dir.as_os_str().as_encoded_bytes().len()),
            )
        {
            return Vec::new();
        }
        config_dir.to_path_buf()
    };
    let Some(paths) = compiler_options
        .get("paths")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let mut mappings = Vec::new();
    for (pattern, targets) in paths {
        let mapping_weight = std::mem::size_of::<Vue3TsconfigPathMapping>()
            .saturating_add(pattern.len())
            .saturating_add(target_base_dir.as_os_str().as_encoded_bytes().len())
            .saturating_add(template_config_dir.as_os_str().as_encoded_bytes().len());
        if !type_resolver
            .external_type_session
            .claim_tsconfig_materialization(mapping_weight)
        {
            return Vec::new();
        }
        let Some(targets) = vue3_materialize_tsconfig_strings(
            vue3_tsconfig_path_target_values(targets),
            type_resolver,
        ) else {
            return Vec::new();
        };
        mappings.push(Vue3TsconfigPathMapping {
            pattern: pattern.clone(),
            targets,
            target_base_dir: target_base_dir.clone(),
            template_config_dir: template_config_dir.to_path_buf(),
        });
    }
    mappings
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

pub(crate) fn vue3_tsconfig_path_target_values(
    value: &serde_json::Value,
) -> impl Iterator<Item = &str> {
    let values = match value {
        serde_json::Value::Array(targets) => targets.as_slice(),
        serde_json::Value::String(_) => std::slice::from_ref(value),
        _ => &[],
    };
    values.iter().filter_map(serde_json::Value::as_str)
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
        type_resolver,
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
        let resolved = resolve_vue3_metadata_module_specifier_path_with_mode(
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
    resolve_vue3_metadata_module_specifier_path_with_mode(
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
    type_resolver: &Vue3TypeResolverContext,
) -> Option<(usize, String)> {
    let mut best: Option<(usize, usize, String)> = None;
    for (index, pattern) in patterns {
        if !type_resolver
            .external_type_session
            .claim_metadata_match_steps(pattern.len().saturating_add(source.len()))
        {
            return None;
        }
        let Some((prefix_len, capture)) = vue3_tsconfig_path_pattern_capture(pattern, source) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_prefix_len, _, _)| prefix_len > *best_prefix_len)
        {
            best = Some((prefix_len, index, capture));
        }
    }
    best.map(|(_, index, capture)| (index, capture))
}

pub(crate) fn vue3_tsconfig_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    vue3_tsconfig_target_path_with_materialization(
        target_base_dir,
        template_config_dir,
        target,
        false,
        type_resolver,
    )
}

pub(crate) fn vue3_materialized_tsconfig_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    vue3_tsconfig_target_path_with_materialization(
        target_base_dir,
        template_config_dir,
        target,
        true,
        type_resolver,
    )
}

fn vue3_tsconfig_target_path_with_materialization(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    claim_materialization: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !vue3_claim_tsconfig_target_steps(target, template_config_dir, "", type_resolver) {
        return None;
    }
    let target = vue3_tsconfig_expand_config_dir_template(
        target,
        template_config_dir,
        type_resolver,
    )?;
    vue3_tsconfig_path_from_expanded_target(
        target_base_dir,
        &target,
        claim_materialization,
        type_resolver,
    )
}

pub(crate) fn vue3_tsconfig_path_mapping_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    capture: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !vue3_claim_tsconfig_target_steps(target, template_config_dir, capture, type_resolver) {
        return None;
    }
    let target =
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, type_resolver)?;
    let target = vue3_typescript_path_target_substitution(&target, capture, type_resolver)?;
    vue3_tsconfig_path_from_expanded_target(target_base_dir, &target, false, type_resolver)
}

fn vue3_claim_tsconfig_target_steps(
    target: &str,
    template_config_dir: &Path,
    capture: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    if !type_resolver
        .external_type_session
        .claim_metadata_target_steps(target.len())
    {
        return false;
    }
    let expands_config_dir = target.starts_with(VUE3_TSCONFIG_CONFIG_DIR_TEMPLATE);
    let template_config_dir_bytes = template_config_dir.as_os_str().as_encoded_bytes();
    let config_dir_steps = if expands_config_dir {
        // Lossy path conversion can expand one encoded byte into one replacement character.
        template_config_dir_bytes.len().saturating_mul(3)
    } else {
        0
    };
    if config_dir_steps != 0
        && !type_resolver
            .external_type_session
            .claim_metadata_target_steps(config_dir_steps)
    {
        return false;
    }
    let substitutes_capture = !capture.is_empty()
        && (target.contains('*')
            || (expands_config_dir && template_config_dir_bytes.contains(&b'*')));
    if substitutes_capture
        && !type_resolver
            .external_type_session
            .claim_metadata_target_steps(capture.len())
    {
        return false;
    }
    true
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

fn vue3_tsconfig_path_from_expanded_target(
    target_base_dir: &Path,
    target: &str,
    claim_materialization: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target = vue3_normalize_typescript_path_separators(target, type_resolver)?;
    let path_bytes = if claim_materialization {
        Some(vue3_typescript_path_materialization_bytes(
            target_base_dir,
            &target,
        )?)
    } else {
        None
    };
    if path_bytes.is_some_and(|path_bytes| {
        !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver)
    }) {
        return None;
    }
    let path = vue3_materialize_normalized_typescript_path(target_base_dir, &target)?;
    debug_assert!(path_bytes.is_none_or(|path_bytes| {
        path.as_os_str().as_encoded_bytes().len() <= path_bytes
    }));
    type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&path))
        .then_some(path)
}

fn vue3_typescript_path_materialization_bytes(
    base_dir: &Path,
    target: &str,
) -> Option<usize> {
    match vue3_typescript_path_kind(target) {
        Vue3TypeScriptPathKind::Relative => {
            Some(vue3_ancestor_search_candidate_weight(base_dir, target))
        }
        Vue3TypeScriptPathKind::Rooted => {
            #[cfg(windows)]
            {
                Some(vue3_ancestor_search_candidate_weight(base_dir, target))
            }
            #[cfg(not(windows))]
            {
                Some(target.len())
            }
        }
        Vue3TypeScriptPathKind::WindowsDriveAbsolute
        | Vue3TypeScriptPathKind::WindowsUncAbsolute => {
            #[cfg(windows)]
            {
                Some(target.len())
            }
            #[cfg(not(windows))]
            {
                None
            }
        }
        Vue3TypeScriptPathKind::Unsupported => None,
    }
}

pub(crate) fn vue3_tsconfig_expand_config_dir_template(
    target: &str,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    let Some(suffix) = target.strip_prefix(VUE3_TSCONFIG_CONFIG_DIR_TEMPLATE) else {
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
    for phase in [
        Vue3PackageResolutionPhase::Types,
        Vue3PackageResolutionPhase::JavaScript,
    ] {
        for node_modules in vue3_node_modules_search_paths(filename, type_resolver) {
            let package_dir = node_modules.join(package_name);
            let is_package_dir = type_resolver
                .external_type_session
                .metadata_path_is_dir(&package_dir)?;
            if is_package_dir {
                let resolved = resolve_vue3_package_entry_phase_with_mode(
                    &package_dir,
                    subpath,
                    resolution_mode,
                    phase,
                    type_resolver,
                );
                if type_resolver.external_type_session.metadata_is_blocked() {
                    return None;
                }
                if let Some(resolved) = resolved {
                    return Some(resolved);
                }
            }
            if phase == Vue3PackageResolutionPhase::Types {
                let types_package_dir =
                    node_modules.join(vue3_at_types_package_name(package_name));
                let is_types_package_dir = type_resolver
                    .external_type_session
                    .metadata_path_is_dir(&types_package_dir)?;
                if is_types_package_dir {
                    let resolved = resolve_vue3_package_entry_phase_with_mode(
                        &types_package_dir,
                        subpath,
                        resolution_mode,
                        phase,
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
        }
    }
    None
}

pub(crate) fn resolve_vue3_classic_type_import_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if source.is_empty()
        || vue3_type_import_source_is_relative(source)
        || type_resolver.external_type_session.metadata_is_blocked()
        || !type_resolver
            .external_type_session
            .metadata_path_is_within_limit(source)
    {
        return None;
    }
    let normalized_source = source.replace('\\', "/");
    for directory in Vue3AncestorSearchPaths::directories(
        Path::new(filename).parent(),
        &type_resolver.external_type_session,
    ) {
        let candidate = normalize_path_components(directory.join(&normalized_source));
        let resolved = resolve_vue3_metadata_module_specifier_path_with_mode(
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
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }

    let (package_name, subpath) = vue3_package_import_parts(source)?;
    for node_modules in vue3_node_modules_search_paths(filename, type_resolver) {
        let types_package_dir = node_modules.join(vue3_at_types_package_name(package_name));
        if !type_resolver
            .external_type_session
            .metadata_path_is_dir(&types_package_dir)?
        {
            continue;
        }
        let resolved = resolve_vue3_package_entry_phase_with_mode(
            &types_package_dir,
            subpath,
            resolution_mode,
            Vue3PackageResolutionPhase::Types,
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

pub(crate) fn vue3_package_import_parts(source: &str) -> Option<(&str, Option<&str>)> {
    if source.starts_with('.') || source.starts_with('/') || source.starts_with('#') {
        return None;
    }

    let segment_is_invalid = |segment: &str| {
        segment.is_empty()
            || matches!(segment, "." | "..")
            || segment.contains(':')
            || segment.contains('\\')
    };
    let mut segments = source.split('/');
    let first = segments.next()?;
    // Bare package fallback must not normalize outside the selected package root.
    if segment_is_invalid(first) {
        return None;
    }

    if first.starts_with('@') {
        let package = segments.next()?;
        if first.len() <= 1 || segment_is_invalid(package) {
            return None;
        }
        let package_name_len = first
            .len()
            .checked_add(1)?
            .checked_add(package.len())?;
        if segments.any(segment_is_invalid) {
            return None;
        }
        let subpath = (package_name_len < source.len())
            .then(|| &source[package_name_len.saturating_add(1)..]);
        return Some((&source[..package_name_len], subpath));
    }

    if segments.any(segment_is_invalid) {
        return None;
    }
    let subpath = (first.len() < source.len()).then(|| &source[first.len().saturating_add(1)..]);
    Some((first, subpath))
}

pub(crate) fn vue3_ancestor_search_candidate_weight(dir: &Path, suffix: &str) -> usize {
    dir.as_os_str()
        .as_encoded_bytes()
        .len()
        .saturating_add(usize::from(
            !dir.as_os_str().is_empty() && !suffix.is_empty(),
        ))
        .saturating_add(suffix.len())
}

pub(crate) fn vue3_claim_tsconfig_path_materialization(
    path_bytes: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    if path_bytes
        > type_resolver
            .external_type_session
            .limits()
            .max_generated_path_bytes
    {
        type_resolver.external_type_session.block_metadata();
        return false;
    }
    type_resolver
        .external_type_session
        .claim_tsconfig_materialization(
            std::mem::size_of::<PathBuf>().saturating_add(path_bytes),
        )
}

struct Vue3AncestorSearchPaths<'a> {
    current: Option<&'a Path>,
    suffix: &'static str,
    alternate_suffix: Option<&'static str>,
    emit_alternate_suffix: bool,
    remaining_depth: usize,
    stop_before_node_modules: bool,
    stop_after_node_modules: bool,
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
            alternate_suffix: None,
            emit_alternate_suffix: false,
            remaining_depth: session.max_ancestor_search_depth(),
            stop_before_node_modules: false,
            stop_after_node_modules: false,
            session,
        }
    }

    fn with_alternate_suffix(mut self, suffix: &'static str) -> Self {
        self.alternate_suffix = Some(suffix);
        self
    }

    fn package_scope(mut self) -> Self {
        self.stop_before_node_modules = true;
        self
    }

    fn project_config(mut self) -> Self {
        self.stop_after_node_modules = true;
        self
    }

    fn directories(
        current: Option<&'a Path>,
        session: &'a Vue3ExternalTypeLoadSession,
    ) -> Self {
        Self::new(current, "", session)
    }
}

impl Iterator for Vue3AncestorSearchPaths<'_> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        let dir = self.current.take()?;
        let is_alternate = self.emit_alternate_suffix;
        let is_node_modules = dir
            .file_name()
            .is_some_and(vue3_path_component_is_node_modules);
        if !is_alternate
            && self.stop_before_node_modules
            && is_node_modules
        {
            return None;
        }
        if !is_alternate && self.remaining_depth == 0 {
            self.session.block_metadata();
            return None;
        }
        if !is_alternate {
            self.remaining_depth -= 1;
        }
        let suffix = if is_alternate {
            self.alternate_suffix
                .expect("alternate suffix emission requires a suffix")
        } else {
            self.suffix
        };
        let finished_dir = is_alternate || self.alternate_suffix.is_none();
        if finished_dir {
            self.current = (!self.stop_after_node_modules || !is_node_modules)
                .then(|| dir.parent())
                .flatten();
            self.emit_alternate_suffix = false;
        } else {
            self.current = Some(dir);
            self.emit_alternate_suffix = true;
        }
        if !self.session.claim_ancestor_search_dir(dir, suffix) {
            self.current = None;
            self.emit_alternate_suffix = false;
            return None;
        }
        Some(normalize_path_components(if suffix.is_empty() {
            dir.to_path_buf()
        } else {
            dir.join(suffix)
        }))
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
        Vue3PackageScopeResolution::Missing => Some(Vue3PackageModuleType::Unspecified),
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
    if let Some(options) = vue3_tsconfig_type_resolver_options(filename, &type_resolver) {
        type_resolver.module_resolution = options.module_resolution;
        type_resolver.module = Some(options.module);
        type_resolver.module_suffixes = options.module_suffixes;
        type_resolver.root_dirs = options.root_dirs;
        type_resolver.allow_js = options.allow_js;
        type_resolver.custom_conditions = options.custom_conditions;
        type_resolver.resolve_package_json_exports = options.resolve_package_json_exports;
        type_resolver.resolve_package_json_imports = options.resolve_package_json_imports;
    } else {
        type_resolver.module_suffixes = std::sync::Arc::from(Vec::<String>::new());
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

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn resolve_vue3_package_type_entry_with_mode(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let resolved = resolve_vue3_package_entry_phase_with_mode(
        package_dir,
        subpath,
        resolution_mode,
        Vue3PackageResolutionPhase::Types,
        type_resolver,
    );
    if resolved.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
        return resolved;
    }
    resolve_vue3_package_entry_phase_with_mode(
        package_dir,
        subpath,
        resolution_mode,
        Vue3PackageResolutionPhase::JavaScript,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_package_entry_phase_with_mode(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let uses_node_esm_specifier_rules = type_resolver
        .module_resolution
        .uses_node_esm_specifier_rules(resolution_mode, &type_resolver.typescript_version);
    match vue3_package_subpath_lookup(package_dir, subpath, type_resolver) {
        Vue3PackageSubpathLookup::Nested => {
            let fallback = Vue3PackagePathFallback {
                allowed: true,
                allow_nested_manifest: true,
                allow_index: true,
            };
            return resolve_vue3_package_path_fallback_for_phase(
                package_dir,
                subpath,
                resolution_mode,
                phase,
                fallback,
                uses_node_esm_specifier_rules,
                type_resolver,
            );
        }
        Vue3PackageSubpathLookup::Root => {}
        Vue3PackageSubpathLookup::Blocked => return None,
    }
    let phase_resolution = resolve_vue3_package_json_entry_phase_with_mode(
        package_dir,
        subpath,
        resolution_mode,
        phase,
        type_resolver,
    );
    let fallback = match phase_resolution {
        Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
        Vue3PackageJsonPhaseResolution::Blocked => return None,
        Vue3PackageJsonPhaseResolution::NoPackageJson => {
            if subpath.is_none() && uses_node_esm_specifier_rules {
                return None;
            }
            Vue3PackagePathFallback {
                allowed: true,
                allow_nested_manifest: true,
                allow_index: true,
            }
        }
        Vue3PackageJsonPhaseResolution::Missing(fallback) => fallback,
    };
    resolve_vue3_package_path_fallback_for_phase(
        package_dir,
        subpath,
        resolution_mode,
        phase,
        fallback,
        uses_node_esm_specifier_rules,
        type_resolver,
    )
}

enum Vue3PackageSubpathLookup {
    Root,
    Nested,
    Blocked,
}

fn vue3_package_subpath_lookup(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageSubpathLookup {
    let Some(subpath) = subpath else {
        return Vue3PackageSubpathLookup::Root;
    };
    let candidate = normalize_path_components(package_dir.join(subpath));
    let nested_package_json = candidate.join("package.json");
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&nested_package_json))
    {
        return Vue3PackageSubpathLookup::Blocked;
    }
    let nested_manifest = type_resolver
        .external_type_session
        .package_json_from_path(&nested_package_json);
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageSubpathLookup::Blocked;
    }
    if nested_manifest.is_none() {
        return Vue3PackageSubpathLookup::Root;
    }
    if !type_resolver.package_json_features().exports {
        return Vue3PackageSubpathLookup::Nested;
    }

    let root_package_json = package_dir.join("package.json");
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&root_package_json))
    {
        return Vue3PackageSubpathLookup::Blocked;
    }
    let root_manifest = type_resolver
        .external_type_session
        .package_json_from_path(&root_package_json);
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageSubpathLookup::Blocked;
    }
    if root_manifest.is_some_and(|manifest| manifest.exports.is_some()) {
        Vue3PackageSubpathLookup::Root
    } else {
        Vue3PackageSubpathLookup::Nested
    }
}

fn resolve_vue3_package_path_fallback_for_phase(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    fallback: Vue3PackagePathFallback,
    uses_node_esm_specifier_rules: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !fallback.allowed {
        return None;
    }
    if let Some(subpath) = subpath {
        let candidate = normalize_path_components(package_dir.join(subpath));
        return match phase {
            Vue3PackageResolutionPhase::Types => {
                resolve_vue3_metadata_bare_package_type_fallback_path_with_mode(
                    &candidate,
                    package_dir,
                    resolution_mode,
                    fallback.allow_nested_manifest,
                    type_resolver,
                )
            }
            Vue3PackageResolutionPhase::JavaScript => {
                resolve_vue3_metadata_bare_package_javascript_fallback_path_with_mode(
                    &candidate,
                    package_dir,
                    resolution_mode,
                    fallback.allow_nested_manifest,
                    type_resolver,
                )
            }
        };
    }
    if !fallback.allow_index {
        return None;
    }
    let (candidate, policy) = if uses_node_esm_specifier_rules {
        (
            package_dir.join("index.js"),
            Vue3PackageTargetPathPolicy::RequireExplicitFileName,
        )
    } else {
        (
            package_dir.join("index"),
            Vue3PackageTargetPathPolicy::AllowImplicit,
        )
    };
    match phase {
        Vue3PackageResolutionPhase::Types => {
            resolve_vue3_metadata_legacy_package_type_field_path_with_mode(
                &candidate,
                resolution_mode,
                policy,
                type_resolver,
            )
        }
        Vue3PackageResolutionPhase::JavaScript => {
            resolve_vue3_metadata_legacy_package_javascript_field_path(
                &candidate,
                policy,
                type_resolver,
            )
        }
    }
}
