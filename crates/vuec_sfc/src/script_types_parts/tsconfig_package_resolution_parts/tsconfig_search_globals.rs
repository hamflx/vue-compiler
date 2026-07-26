#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigPathMapping {
    pub(crate) pattern: String,
    pub(crate) targets: Vec<String>,
    pub(crate) target_base_dir: PathBuf,
    pub(crate) template_config_dir: PathBuf,
}

#[derive(Debug, Default)]
struct Vue3TsconfigModuleResolutionSettings {
    path_mappings: Option<Vec<Vue3TsconfigPathMapping>>,
    paths_base_dir: Option<PathBuf>,
    base_url: Option<PathBuf>,
    base_url_is_declared: bool,
}

impl Vue3TsconfigModuleResolutionSettings {
    fn inherit(&mut self, mut inherited: Self) {
        if inherited.path_mappings.is_some() {
            self.path_mappings = inherited.path_mappings.take();
            self.paths_base_dir = inherited.paths_base_dir.take();
        }
        if inherited.base_url_is_declared {
            self.base_url = inherited.base_url;
            self.base_url_is_declared = true;
        }
    }

    fn apply_effective_paths_base(&mut self, typescript_version: &nodejs_semver::Version) {
        let target_base_dir = if typescript_version < &(7, 0, 0).into() {
            self.base_url.as_ref()
        } else {
            None
        }
        .or(self.paths_base_dir.as_ref());
        let Some(target_base_dir) = target_base_dir else {
            return;
        };
        for mapping in self.path_mappings.iter_mut().flatten() {
            mapping.target_base_dir.clone_from(target_base_dir);
        }
    }

    fn into_parts(self) -> (Vec<Vue3TsconfigPathMapping>, Option<PathBuf>) {
        (self.path_mappings.unwrap_or_default(), self.base_url)
    }
}

type Vue3TsconfigGraphStateKey = (PathBuf, PathBuf, PathBuf);
type Vue3TsconfigTypeRootsOverride = Option<std::sync::Arc<[PathBuf]>>;

#[derive(Clone, Debug)]
struct Vue3TsconfigTypeRoots {
    paths: std::sync::Arc<[PathBuf]>,
    is_explicit: bool,
}

#[derive(Debug, Default)]
struct Vue3TsconfigGraphTraversal {
    seen_states: BTreeSet<Vue3TsconfigGraphStateKey>,
    active_identities: BTreeSet<PathBuf>,
}

fn vue3_tsconfig_graph_state_key(
    config_path: &Path,
    template_config_dir: &Path,
) -> Vue3TsconfigGraphStateKey {
    (
        vue3_external_type_path_identity(config_path),
        vue3_external_type_lexical_path(config_path.parent().unwrap_or_else(|| Path::new(""))),
        vue3_external_type_lexical_path(template_config_dir),
    )
}

fn vue3_tsconfig_graph_enter(
    config_path: &Path,
    template_config_dir: &Path,
    depth: usize,
    traversal: &mut Vue3TsconfigGraphTraversal,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    let identity = state_key.0.clone();
    if traversal.active_identities.contains(&identity) {
        return None;
    }
    if traversal.seen_states.contains(&state_key) {
        return None;
    }
    if depth >= type_resolver.external_type_session.max_tsconfig_depth() {
        type_resolver.external_type_session.block_metadata();
        return None;
    }
    traversal.seen_states.insert(state_key.clone());
    if !type_resolver
        .external_type_session
        .claim_tsconfig_node(&state_key)
    {
        return None;
    }
    traversal.active_identities.insert(identity.clone());
    Some(identity)
}

#[cfg(test)]
pub(crate) fn resolve_vue3_tsconfig_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_tsconfig_type_import_with_mode(
        filename,
        source,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_tsconfig_type_import_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let config_path = vue3_tsconfig_search_paths(filename, type_resolver).next()?;
    let config_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let mut traversal = Vue3TsconfigGraphTraversal::default();
    let settings = vue3_tsconfig_module_resolution_from_config(
        &config_path,
        &config_dir,
        &mut traversal,
        0,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let (path_mappings, base_url) = settings.into_parts();
    let resolved = resolve_vue3_tsconfig_path_mappings_with_mode(
        &path_mappings,
        source,
        resolution_mode,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if resolved.is_some() {
        return resolved;
    }
    if type_resolver.typescript_version < (7, 0, 0).into() {
        if let Some(base_url) = base_url.as_ref() {
            let resolved = resolve_vue3_tsconfig_base_url_with_mode(
                base_url,
                source,
                resolution_mode,
                type_resolver,
            );
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if resolved.is_some() {
                return resolved;
            }
        }
    }
    None
}

pub(crate) fn vue3_tsconfig_search_paths<'a>(
    filename: &'a str,
    type_resolver: &'a Vue3TypeResolverContext,
) -> impl Iterator<Item = PathBuf> + 'a {
    Vue3AncestorSearchPaths::new(
        Path::new(filename).parent(),
        "tsconfig.json",
        &type_resolver.external_type_session,
    )
    .filter(|candidate| {
        type_resolver
            .external_type_session
            .metadata_path_is_file(candidate)
            .unwrap_or(false)
    })
}

#[derive(Debug, Default)]
struct Vue3TsconfigTypeRootsTraversal {
    active_identities: BTreeSet<PathBuf>,
    cached_overrides: BTreeMap<Vue3TsconfigGraphStateKey, Vue3TsconfigTypeRootsOverride>,
}

#[cfg(test)]
fn resolve_vue3_type_reference_directive(
    project_filename: &str,
    containing_filename: &str,
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_reference_directive_with_mode(
        project_filename,
        containing_filename,
        type_name,
        None,
        type_resolver,
    )
}

fn resolve_vue3_type_reference_directive_with_mode(
    project_filename: &str,
    containing_filename: &str,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_name.is_empty() {
        return None;
    }
    let cache_source = format!(
        "{}:{containing_filename}{type_name}",
        containing_filename.len()
    );
    match type_resolver
        .external_type_session
        .begin_type_import_resolution(
            Vue3TypeResolutionKind::ReferenceTypes(resolution_mode),
            project_filename,
            &cache_source,
            &type_resolver.typescript_version,
            false,
        ) {
        Vue3TypeImportResolutionLoad::Ready(resolution) => resolution,
        Vue3TypeImportResolutionLoad::Failed => None,
        Vue3TypeImportResolutionLoad::Start {
            cache_key,
            failure_epoch,
        } => {
            let resolution = resolve_vue3_type_reference_directive_uncached(
                project_filename,
                containing_filename,
                type_name,
                resolution_mode,
                type_resolver,
            );
            type_resolver
                .external_type_session
                .finish_type_import_resolution(cache_key, resolution, failure_epoch, false)
        }
    }
}

fn resolve_vue3_type_reference_directive_uncached(
    project_filename: &str,
    containing_filename: &str,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let normalized_type_name = type_name.replace('\\', "/");
    let type_roots = vue3_tsconfig_effective_type_roots(project_filename, type_resolver)?;
    let primary = resolve_vue3_tsconfig_named_type_global_type_file(
        &type_roots,
        &normalized_type_name,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if primary.is_some() {
        return primary;
    }
    let secondary = if vue3_type_reference_name_is_relative_or_rooted(&normalized_type_name) {
        let base = Path::new(containing_filename)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let candidate = normalize_path_components(base.join(normalized_type_name));
        resolve_vue3_type_reference_package_candidate(&candidate, None, true, None, type_resolver)
    } else {
        resolve_vue3_bare_type_reference(
            containing_filename,
            type_name,
            resolution_mode,
            type_resolver,
        )
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        None
    } else {
        secondary
    }
}

fn vue3_type_reference_name_is_relative_or_rooted(type_name: &str) -> bool {
    type_name == "."
        || type_name == ".."
        || vue3_type_import_source_is_relative(type_name)
        || Path::new(type_name).has_root()
}

fn vue3_tsconfig_effective_type_roots(
    project_filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigTypeRoots> {
    let config_path = vue3_tsconfig_search_paths(project_filename, type_resolver).next();
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let project_dir = Path::new(project_filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let Some(config_path) = config_path else {
        let roots = vue3_tsconfig_default_type_roots(project_dir, type_resolver);
        return (!type_resolver.external_type_session.metadata_is_blocked()).then(|| {
            Vue3TsconfigTypeRoots {
                paths: std::sync::Arc::from(roots),
                is_explicit: false,
            }
        });
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let mut traversal = Vue3TsconfigTypeRootsTraversal::default();
    let configured = vue3_tsconfig_type_roots_override_from_config(
        &config_path,
        config_dir,
        &mut traversal,
        0,
        type_resolver,
    )?;
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if let Some(configured) = configured {
        return Some(Vue3TsconfigTypeRoots {
            paths: configured,
            is_explicit: true,
        });
    }
    let roots = vue3_tsconfig_default_type_roots(config_dir, type_resolver);
    (!type_resolver.external_type_session.metadata_is_blocked()).then(|| Vue3TsconfigTypeRoots {
        paths: std::sync::Arc::from(roots),
        is_explicit: false,
    })
}

fn vue3_tsconfig_type_roots_override_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigTypeRootsTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigTypeRootsOverride> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    let identity = state_key.0.clone();
    if traversal.active_identities.contains(&identity) {
        return Some(None);
    }
    if let Some(cached) = traversal.cached_overrides.get(&state_key) {
        return Some(cached.clone());
    }
    if depth >= type_resolver.external_type_session.max_tsconfig_depth() {
        type_resolver.external_type_session.block_metadata();
        return None;
    }
    if !type_resolver
        .external_type_session
        .claim_tsconfig_node(&state_key)
    {
        return None;
    }
    traversal.active_identities.insert(identity.clone());
    let resolved = (|| {
        let value = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let extended_paths = vue3_tsconfig_extends_paths(&value, config_dir, type_resolver);
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        let mut effective = None;
        for extended in extended_paths {
            if let Some(extended_roots) = vue3_tsconfig_type_roots_override_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            )? {
                effective = Some(extended_roots);
            }
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        let direct = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
            .and_then(|options| options.get("typeRoots"));
        if let Some(direct) = direct {
            let mut roots = Vec::new();
            for target in vue3_tsconfig_string_array(Some(direct)) {
                if !type_resolver
                    .external_type_session
                    .claim_tsconfig_discovery_entry()
                {
                    return None;
                }
                let path = vue3_tsconfig_target_path(
                    config_dir,
                    template_config_dir,
                    &target,
                    type_resolver,
                )?;
                roots.push(path);
            }
            effective = Some(std::sync::Arc::from(roots));
        }
        Some(effective)
    })();
    traversal.active_identities.remove(&identity);
    if let Some(effective) = &resolved {
        traversal
            .cached_overrides
            .insert(state_key, effective.clone());
    }
    resolved
}

fn resolve_vue3_tsconfig_named_type_global_type_file(
    type_roots: &Vue3TsconfigTypeRoots,
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_name.is_empty() {
        return None;
    }
    for type_root in type_roots.paths.iter() {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return None;
        }
        let scoped_default_name = if type_resolver.typescript_version >= (5, 1, 0).into()
            && vue3_type_root_uses_scoped_package_mangling(type_root)
        {
            vue3_mangle_scoped_package_name(type_name)
        } else {
            None
        };
        let package_name = scoped_default_name.as_deref().unwrap_or(type_name);
        let package_dir = normalize_path_components(type_root.join(package_name));
        let file = resolve_vue3_type_reference_package_candidate(
            &package_dir,
            None,
            type_roots.is_explicit,
            None,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if let Some(file) = file {
            return type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
                .then_some(file);
        }
    }
    None
}

fn vue3_type_root_uses_scoped_package_mangling(type_root: &Path) -> bool {
    let mut components = type_root.components().rev();
    matches!(
        (components.next(), components.next()),
        (
            Some(std::path::Component::Normal(at_types)),
            Some(std::path::Component::Normal(node_modules)),
        ) if at_types
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("@types"))
            && node_modules
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("node_modules"))
    )
}

fn vue3_mangle_scoped_package_name(type_name: &str) -> Option<String> {
    let scoped = type_name.strip_prefix('@')?;
    let mangled = scoped.replacen('/', "__", 1);
    (mangled != scoped).then_some(mangled)
}

fn resolve_vue3_bare_type_reference(
    containing_filename: &str,
    type_name: &str,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let (package_name, subpath) = vue3_package_import_parts(type_name)?;
    for node_modules in vue3_node_modules_search_paths(containing_filename, type_resolver) {
        let package_dir = node_modules.join(&package_name);
        let resolved = resolve_vue3_type_reference_package_candidate(
            &package_dir,
            subpath.as_deref(),
            true,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if resolved.is_some() {
            return resolved;
        }
        let types_package_dir = node_modules.join(vue3_at_types_package_name(&package_name));
        let resolved = resolve_vue3_type_reference_package_candidate(
            &types_package_dir,
            subpath.as_deref(),
            true,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if resolved.is_some() {
            return resolved;
        }
    }
    None
}

fn resolve_vue3_type_reference_package_candidate(
    package_dir: &Path,
    subpath: Option<&str>,
    allow_direct_file: bool,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let candidate = subpath
        .map(|subpath| package_dir.join(subpath))
        .unwrap_or_else(|| package_dir.to_path_buf());
    if subpath.is_none() && allow_direct_file {
        let direct =
            resolve_vue3_metadata_type_reference_declaration_file(&candidate, type_resolver);
        if direct.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return direct;
        }
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_dir(package_dir)?
    {
        return None;
    }
    match resolve_vue3_package_json_type_reference_entry(
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
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if subpath.is_some() {
        let direct =
            resolve_vue3_metadata_type_reference_declaration_file(&candidate, type_resolver);
        if direct.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return direct;
        }
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_dir(&candidate)?
    {
        return None;
    }
    resolve_vue3_metadata_type_reference_declaration_file(
        &candidate.join("index"),
        type_resolver,
    )
}

fn vue3_tsconfig_module_resolution_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigGraphTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3TsconfigModuleResolutionSettings {
    let Some(identity) = vue3_tsconfig_graph_enter(
        config_path,
        template_config_dir,
        depth,
        traversal,
        type_resolver,
    ) else {
        return Vue3TsconfigModuleResolutionSettings::default();
    };
    let settings = (|| {
        let value = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let mut settings = Vue3TsconfigModuleResolutionSettings::default();
        for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
            settings.inherit(vue3_tsconfig_module_resolution_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            ));
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Some(Vue3TsconfigModuleResolutionSettings::default());
        }
        if vue3_tsconfig_declares_compiler_option(&value, "baseUrl") {
            settings.base_url = vue3_tsconfig_direct_base_url(
                &value,
                config_dir,
                template_config_dir,
                type_resolver,
            );
            settings.base_url_is_declared = true;
        }
        if vue3_tsconfig_declares_compiler_option(&value, "paths") {
            settings.path_mappings = Some(vue3_tsconfig_direct_path_mappings(
                &value,
                config_dir,
                template_config_dir,
                type_resolver,
            ));
            settings.paths_base_dir = Some(config_dir.to_path_buf());
        }
        settings.apply_effective_paths_base(&type_resolver.typescript_version);
        Some(settings)
    })()
    .unwrap_or_default();
    traversal.active_identities.remove(&identity);
    settings
}

pub(crate) fn vue3_tsconfig_global_type_files(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut traversal = Vue3TsconfigGraphTraversal::default();
    let mut seen_files = BTreeSet::new();
    for config_path in vue3_tsconfig_search_paths(filename, type_resolver) {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &config_path,
            config_dir,
            &mut traversal,
            &mut seen_files,
            &mut files,
            type_resolver,
            0,
        );
    }
    if type_resolver.external_type_session.metadata_is_blocked() {
        Vec::new()
    } else {
        files
    }
}

fn vue3_tsconfig_global_type_files_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigGraphTraversal,
    seen_files: &mut BTreeSet<String>,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    depth: usize,
) {
    let Some(identity) = vue3_tsconfig_graph_enter(
        config_path,
        template_config_dir,
        depth,
        traversal,
        type_resolver,
    ) else {
        return;
    };
    let Some(value) = type_resolver
        .external_type_session
        .tsconfig_from_path(config_path)
    else {
        traversal.active_identities.remove(&identity);
        return;
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
        vue3_tsconfig_global_type_files_from_config(
            &extended,
            template_config_dir,
            traversal,
            seen_files,
            files,
            type_resolver,
            depth + 1,
        );
    }
    if type_resolver.external_type_session.metadata_is_blocked() {
        traversal.active_identities.remove(&identity);
        return;
    }
    for file in vue3_tsconfig_direct_global_type_files(
        &value,
        config_dir,
        template_config_dir,
        type_resolver,
    ) {
        let normalized = normalize_path_string(&file);
        if seen_files.insert(normalized) {
            files.push(file);
        }
    }
    for reference in vue3_tsconfig_reference_paths(&value, config_dir, type_resolver) {
        let reference_dir = reference.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &reference,
            reference_dir,
            traversal,
            seen_files,
            files,
            type_resolver,
            depth + 1,
        );
    }
    traversal.active_identities.remove(&identity);
}

pub(crate) fn vue3_tsconfig_direct_global_type_files(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for target in vue3_tsconfig_string_array(value.get("files")) {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return Vec::new();
        }
        let Some(path) =
            vue3_tsconfig_target_path(config_dir, template_config_dir, &target, type_resolver)
        else {
            return Vec::new();
        };
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
            {
                return Vec::new();
            }
            files.push(path);
        }
    }
    let Some(exclude_patterns) = vue3_tsconfig_exclude_patterns(
        value,
        config_dir,
        template_config_dir,
        type_resolver,
    ) else {
        return Vec::new();
    };
    let exclude_matchers = exclude_patterns
        .iter()
        .map(|pattern| Vue3CompiledTsconfigGlob::new(pattern))
        .collect::<Vec<_>>();
    for target in vue3_tsconfig_string_array(value.get("include")) {
        files.extend(vue3_tsconfig_include_global_type_files_with_excludes(
            config_dir,
            template_config_dir,
            &target,
            &exclude_matchers,
            type_resolver,
        ));
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files.extend(vue3_tsconfig_compiler_option_global_type_files(
        value,
        config_dir,
        template_config_dir,
        type_resolver,
    ));
    if type_resolver.external_type_session.metadata_is_blocked() {
        Vec::new()
    } else {
        files
    }
}

pub(crate) fn vue3_tsconfig_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}

pub(crate) fn vue3_tsconfig_compiler_option_global_type_files(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let compiler_options = value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object);
    let configured_types = compiler_options
        .and_then(|options| options.get("types"))
        .map(|types| vue3_tsconfig_string_array(Some(types)));
    if configured_types.as_ref().is_some_and(Vec::is_empty) {
        return Vec::new();
    }
    let has_configured_type_roots =
        compiler_options.is_some_and(|options| options.get("typeRoots").is_some());
    let mut configured_type_roots = Vec::new();
    for target in
        vue3_tsconfig_string_array(compiler_options.and_then(|options| options.get("typeRoots")))
    {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return Vec::new();
        }
        let Some(path) =
            vue3_tsconfig_target_path(config_dir, template_config_dir, &target, type_resolver)
        else {
            return Vec::new();
        };
        if path.is_dir() {
            configured_type_roots.push(path);
        }
    }
    let type_roots = if has_configured_type_roots {
        configured_type_roots
    } else {
        vue3_tsconfig_default_type_roots(config_dir, type_resolver)
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    if let Some(types) = configured_types {
        let mut files = Vec::new();
        for type_name in types {
            files.extend(vue3_tsconfig_named_type_global_type_files(
                &type_roots,
                &type_name,
                type_resolver,
            ));
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vec::new();
            }
        }
        return files;
    }
    let mut files = Vec::new();
    for type_root in type_roots {
        files.extend(vue3_tsconfig_all_type_root_global_type_files(
            &type_root,
            type_resolver,
        ));
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_default_type_roots(
    config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut type_roots = Vec::new();
    for node_modules in vue3_node_modules_search_paths_from_dir(config_dir, type_resolver) {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return Vec::new();
        }
        let path = normalize_path_components(node_modules.join("@types"));
        if path.is_dir() {
            type_roots.push(path);
        }
    }
    type_roots
}

pub(crate) fn vue3_tsconfig_named_type_global_type_files(
    type_roots: &[PathBuf],
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_type_name_is_safe(type_name) {
        return Vec::new();
    }
    let mut files = Vec::new();
    for type_root in type_roots {
        for package_dir in vue3_tsconfig_type_name_package_dirs(type_root, type_name) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_entry()
            {
                return Vec::new();
            }
            if let Some(file) =
                vue3_tsconfig_type_package_global_type_file(&package_dir, type_resolver)
            {
                files.push(file);
            }
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vec::new();
            }
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_type_name_is_safe(type_name: &str) -> bool {
    !type_name.is_empty()
        && !type_name.contains(':')
        && !type_name.contains('\\')
        && !Path::new(type_name).is_absolute()
        && !type_name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

pub(crate) fn vue3_tsconfig_type_name_package_dirs(
    type_root: &Path,
    type_name: &str,
) -> Vec<PathBuf> {
    if let Some(scoped) = type_name.strip_prefix('@') {
        let parts = scoped.split('/').collect::<Vec<_>>();
        if parts.len() == 2 {
            return vec![
                normalize_path_components(type_root.join(format!("@{}", parts[0])).join(parts[1])),
                normalize_path_components(type_root.join(parts[0]).join(parts[1])),
                normalize_path_components(type_root.join(format!("{}__{}", parts[0], parts[1]))),
            ];
        }
    }
    vec![normalize_path_components(type_root.join(type_name))]
}

pub(crate) fn vue3_tsconfig_all_type_root_global_type_files(
    type_root: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Some(entries) = vue3_tsconfig_bounded_sorted_dir_entries(type_root, type_resolver) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !entry.is_dir() || name.is_empty() || name.starts_with('.') {
            continue;
        }
        if name.starts_with('@') {
            files.extend(vue3_tsconfig_all_scoped_type_root_global_type_files(
                &entry,
                type_resolver,
            ));
        } else if let Some(file) =
            vue3_tsconfig_type_package_global_type_file(&entry, type_resolver)
        {
            files.push(file);
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_all_scoped_type_root_global_type_files(
    scope_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Some(entries) = vue3_tsconfig_bounded_sorted_dir_entries(scope_dir, type_resolver) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries {
        if !entry.is_dir()
            || !entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.is_empty() && !name.starts_with('.'))
        {
            continue;
        }
        if let Some(file) = vue3_tsconfig_type_package_global_type_file(&entry, type_resolver) {
            files.push(file);
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vec::new();
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_type_package_global_type_file(
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let path = resolve_vue3_package_type_entry(package_dir, None, type_resolver)?;
    if !vue3_tsconfig_global_type_file_is_supported(&path) {
        return None;
    }
    type_resolver
        .external_type_session
        .claim_tsconfig_discovery_file()
        .then_some(path)
}

fn vue3_tsconfig_bounded_sorted_dir_entries(
    dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vec<PathBuf>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Some(Vec::new());
    };
    let mut paths = Vec::new();
    for entry in entries {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return None;
        }
        if let Ok(entry) = entry {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Some(paths)
}

pub(crate) fn vue3_tsconfig_global_type_file_is_supported(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                [".d.ts", ".d.mts", ".d.cts"]
                    .iter()
                    .any(|extension| name.ends_with(extension))
            })
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_include_global_type_files(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    vue3_tsconfig_include_global_type_files_with_excludes(
        config_dir,
        template_config_dir,
        target,
        &[],
        type_resolver,
    )
}

fn vue3_tsconfig_include_global_type_files_with_excludes(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    exclude_matchers: &[Vue3CompiledTsconfigGlob<'_>],
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_include_can_match_global_type_files(target) {
        return Vec::new();
    }
    if !type_resolver
        .external_type_session
        .claim_tsconfig_discovery_entry()
    {
        return Vec::new();
    }
    if !target.contains('*') && !target.contains('?') {
        let Some(path) =
            vue3_tsconfig_target_path(config_dir, template_config_dir, target, type_resolver)
        else {
            return Vec::new();
        };
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
            {
                return Vec::new();
            }
            return vue3_tsconfig_filter_global_type_files(
                vec![path],
                None,
                exclude_matchers,
                type_resolver,
            );
        }
        if path.is_dir() {
            let mut files = Vec::new();
            vue3_collect_global_type_files_from_dir(&path, &mut files, type_resolver);
            return vue3_tsconfig_filter_global_type_files(
                files,
                None,
                exclude_matchers,
                type_resolver,
            );
        }
        return Vec::new();
    }
    let Some(glob) =
        vue3_tsconfig_include_glob(config_dir, template_config_dir, target, type_resolver)
    else {
        return Vec::new();
    };
    let mut files = Vec::new();
    vue3_collect_global_type_files_from_dir(&glob.root, &mut files, type_resolver);
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    let matcher = Vue3CompiledTsconfigGlob::new(&glob.pattern);
    vue3_tsconfig_filter_global_type_files(
        files,
        Some(&matcher),
        exclude_matchers,
        type_resolver,
    )
}

fn vue3_tsconfig_filter_global_type_files(
    files: Vec<PathBuf>,
    include_matcher: Option<&Vue3CompiledTsconfigGlob<'_>>,
    exclude_matchers: &[Vue3CompiledTsconfigGlob<'_>],
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if include_matcher.is_none() && exclude_matchers.is_empty() {
        return files;
    }
    let mut budget = type_resolver
        .external_type_session
        .tsconfig_glob_match_budget();
    let mut matched = Vec::new();
    for file in files {
        let path = normalize_path_string(&file);
        if let Some(matcher) = include_matcher {
            match matcher.matches(&path, &mut || budget.claim_step()) {
                Some(true) => {}
                Some(false) => continue,
                None => break,
            }
        }
        let mut excluded = false;
        for matcher in exclude_matchers {
            match matcher.matches(&path, &mut || budget.claim_step()) {
                Some(true) => {
                    excluded = true;
                    break;
                }
                Some(false) => {}
                None => break,
            }
        }
        if budget.is_exhausted() {
            break;
        }
        if !excluded {
            matched.push(file);
        }
    }
    if !budget.finish() || type_resolver.external_type_session.metadata_is_blocked() {
        Vec::new()
    } else {
        matched
    }
}

fn vue3_tsconfig_exclude_patterns(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vec<String>> {
    let mut patterns = Vec::new();
    for target in vue3_tsconfig_string_array(value.get("exclude")) {
        if !type_resolver
            .external_type_session
            .claim_tsconfig_discovery_entry()
        {
            return None;
        }
        let path = vue3_tsconfig_include_path(
            config_dir,
            template_config_dir,
            &target,
            type_resolver,
        )?;
        let final_segment = target.rsplit(['/', '\\']).next().unwrap_or(&target);
        let is_directory_pattern = path.is_dir()
            || target.ends_with('/')
            || target.ends_with('\\')
            || !final_segment.contains('.');
        let mut pattern = normalize_path_string(&path);
        if is_directory_pattern {
            pattern = type_resolver.external_type_session.concat_metadata_path(
                pattern.trim_end_matches('/'),
                "/**",
            )?;
        }
        patterns.push(pattern);
    }
    Some(patterns)
}

pub(crate) fn vue3_tsconfig_include_can_match_global_type_files(target: &str) -> bool {
    let file_pattern = target
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(target);
    if !file_pattern.contains('.') {
        return true;
    }
    [".d.ts", ".d.mts", ".d.cts", ".ts", ".mts", ".cts"]
        .iter()
        .any(|extension| file_pattern.ends_with(extension))
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_include_pattern(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    let path = vue3_tsconfig_include_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )?;
    Some(normalize_path_string(&path))
}

fn vue3_tsconfig_include_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let target =
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, type_resolver)?;
    vue3_tsconfig_path_from_expanded_target(config_dir, &target, type_resolver)
}

struct Vue3TsconfigIncludeGlob {
    pattern: String,
    root: PathBuf,
}

fn vue3_tsconfig_include_glob(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigIncludeGlob> {
    let path = vue3_tsconfig_include_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )?;
    let root = vue3_tsconfig_include_root_from_pattern(&path)?;
    Some(Vue3TsconfigIncludeGlob {
        pattern: normalize_path_string(&path),
        root,
    })
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_include_root_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let pattern = vue3_tsconfig_include_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )?;
    vue3_tsconfig_include_root_from_pattern(&pattern)
}

fn vue3_tsconfig_include_root_from_pattern(pattern: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in pattern.components() {
        let contains_wildcard = matches!(
            component,
            std::path::Component::Normal(segment)
                if segment.to_string_lossy().contains(['*', '?'])
        );
        if contains_wildcard {
            break;
        }
        root.push(component.as_os_str());
    }
    if root.as_os_str().is_empty() {
        root.push(".");
    }
    root.is_dir().then_some(root)
}

pub(crate) fn vue3_collect_global_type_files_from_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) {
    let initial_len = files.len();
    let mut seen_dirs = BTreeSet::new();
    let max_depth = type_resolver
        .external_type_session
        .max_tsconfig_discovery_depth();
    vue3_collect_global_type_files_from_dir_inner(
        dir,
        files,
        type_resolver,
        &mut seen_dirs,
        0,
        max_depth,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        files.truncate(initial_len);
    }
}

fn vue3_collect_global_type_files_from_dir_inner(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    seen_dirs: &mut BTreeSet<PathBuf>,
    depth: usize,
    max_depth: usize,
) {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return;
    }
    let canonical_dir =
        std::fs::canonicalize(dir).unwrap_or_else(|_| normalize_path_components(dir.to_path_buf()));
    if !seen_dirs.insert(canonical_dir) {
        return;
    }
    let Some(entries) = vue3_tsconfig_bounded_sorted_dir_entries(dir, type_resolver) else {
        return;
    };
    for path in entries {
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if file_type.is_dir() {
            if depth < max_depth {
                vue3_collect_global_type_files_from_dir_inner(
                    &path,
                    files,
                    type_resolver,
                    seen_dirs,
                    depth + 1,
                    max_depth,
                );
            }
        } else if file_type.is_file() && vue3_tsconfig_global_type_file_is_supported(&path) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
            {
                return;
            }
            files.push(normalize_path_components(path));
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return;
        }
    }
}

struct Vue3CompiledTsconfigGlob<'a> {
    parts: Vec<&'a str>,
}

impl<'a> Vue3CompiledTsconfigGlob<'a> {
    fn new(pattern: &'a str) -> Self {
        Self::from_parts(pattern.split('/'))
    }

    fn from_parts(parts: impl IntoIterator<Item = &'a str>) -> Self {
        let mut compiled = Vec::new();
        for part in parts {
            if part == "**" && compiled.last().copied() == Some("**") {
                continue;
            }
            compiled.push(part);
        }
        Self { parts: compiled }
    }

    fn matches(
        &self,
        path: &str,
        claim_step: &mut impl FnMut() -> bool,
    ) -> Option<bool> {
        let path_parts = path.split('/').collect::<Vec<_>>();
        self.matches_parts(&path_parts, claim_step)
    }

    fn matches_parts(
        &self,
        path: &[&str],
        claim_step: &mut impl FnMut() -> bool,
    ) -> Option<bool> {
        if !claim_step() {
            return None;
        }
        let mut pattern_index = 0;
        let mut path_index = 0;
        let mut double_star_pattern_index = None;
        let mut double_star_path_index = 0;
        while path_index < path.len() {
            if !claim_step() {
                return None;
            }
            if self.parts.get(pattern_index).copied() == Some("**") {
                double_star_pattern_index = Some(pattern_index);
                double_star_path_index = path_index;
                pattern_index += 1;
                continue;
            }
            let segment_matches = match self.parts.get(pattern_index) {
                Some(pattern) => vue3_tsconfig_glob_segment_match_bounded(
                    pattern,
                    path[path_index],
                    claim_step,
                )?,
                None => false,
            };
            if segment_matches {
                pattern_index += 1;
                path_index += 1;
                continue;
            }
            let Some(double_star_index) = double_star_pattern_index else {
                return Some(false);
            };
            if !claim_step() {
                return None;
            }
            double_star_path_index += 1;
            path_index = double_star_path_index;
            pattern_index = double_star_index + 1;
        }
        while self.parts.get(pattern_index).copied() == Some("**") {
            if !claim_step() {
                return None;
            }
            pattern_index += 1;
        }
        Some(pattern_index == self.parts.len())
    }
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern: std::borrow::Cow<'_, str> = if pattern.contains('\\') {
        std::borrow::Cow::Owned(pattern.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(pattern)
    };
    let path: std::borrow::Cow<'_, str> = if path.contains('\\') {
        std::borrow::Cow::Owned(path.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(path)
    };
    Vue3CompiledTsconfigGlob::new(&pattern)
        .matches(&path, &mut || true)
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_matches_with_session(
    pattern: &str,
    path: &str,
    session: &Vue3ExternalTypeLoadSession,
) -> Option<bool> {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let matcher = Vue3CompiledTsconfigGlob::new(&pattern);
    let mut budget = session.tsconfig_glob_match_budget();
    let result = matcher.matches(&path, &mut || budget.claim_step());
    if budget.finish() {
        result
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_parts_match(pattern: &[&str], path: &[&str]) -> bool {
    Vue3CompiledTsconfigGlob::from_parts(pattern.iter().copied())
        .matches_parts(path, &mut || true)
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn vue3_tsconfig_glob_segment_match(pattern: &str, text: &str) -> bool {
    vue3_tsconfig_glob_segment_match_bounded(pattern, text, &mut || true).unwrap_or(false)
}

fn vue3_tsconfig_glob_segment_match_bounded(
    pattern: &str,
    text: &str,
    claim_step: &mut impl FnMut() -> bool,
) -> Option<bool> {
    if !claim_step() {
        return None;
    }
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut star_pattern_index = None;
    let mut star_text_index = 0;
    while text_index < text.len() {
        if !claim_step() {
            return None;
        }
        let pattern_char = vue3_tsconfig_glob_next_char(pattern, pattern_index);
        let (text_char, next_text_index) =
            vue3_tsconfig_glob_next_char(text, text_index).expect("valid glob text index");
        match pattern_char {
            Some(('*', mut next_pattern_index)) => {
                while let Some(('*', next_index)) =
                    vue3_tsconfig_glob_next_char(pattern, next_pattern_index)
                {
                    if !claim_step() {
                        return None;
                    }
                    next_pattern_index = next_index;
                }
                star_pattern_index = Some(next_pattern_index);
                star_text_index = text_index;
                pattern_index = next_pattern_index;
                if pattern_index == pattern.len() {
                    return Some(true);
                }
            }
            Some(('?', next_pattern_index)) => {
                pattern_index = next_pattern_index;
                text_index = next_text_index;
            }
            Some((pattern_char, next_pattern_index)) if pattern_char == text_char => {
                pattern_index = next_pattern_index;
                text_index = next_text_index;
            }
            _ => {
                let Some(retry_pattern_index) = star_pattern_index else {
                    return Some(false);
                };
                if !claim_step() {
                    return None;
                }
                let Some((_, next_star_text_index)) =
                    vue3_tsconfig_glob_next_char(text, star_text_index)
                else {
                    return Some(false);
                };
                star_text_index = next_star_text_index;
                text_index = next_star_text_index;
                pattern_index = retry_pattern_index;
            }
        }
    }
    while let Some(('*', next_pattern_index)) =
        vue3_tsconfig_glob_next_char(pattern, pattern_index)
    {
        if !claim_step() {
            return None;
        }
        pattern_index = next_pattern_index;
    }
    Some(pattern_index == pattern.len())
}

fn vue3_tsconfig_glob_next_char(source: &str, index: usize) -> Option<(char, usize)> {
    let ch = source.get(index..)?.chars().next()?;
    Some((ch, index + ch.len_utf8()))
}

#[cfg(test)]
mod vue3_type_reference_directive_tests {
    use super::*;

    fn write_type_package(type_root: &Path, name: &str) -> PathBuf {
        write_type_package_with_entry(type_root, name, "index.d.ts")
    }

    fn write_type_package_with_entry(
        type_root: &Path,
        name: &str,
        entry_name: &str,
    ) -> PathBuf {
        let package_dir = type_root.join(name);
        std::fs::create_dir_all(&package_dir).expect("create type package directory");
        std::fs::write(
            package_dir.join("package.json"),
            format!(r#"{{"types":"{entry_name}"}}"#),
        )
        .expect("write type package manifest");
        let entry = package_dir.join(entry_name);
        std::fs::write(&entry, "declare interface ReferencedGlobal {}")
            .expect("write type package entry");
        entry
    }

    fn write_conditional_type_package(
        node_modules: &Path,
        name: &str,
        manifest: &str,
        entries: &[&str],
    ) -> PathBuf {
        let package_dir = node_modules.join(name);
        std::fs::create_dir_all(&package_dir).expect("create conditional type package");
        std::fs::write(package_dir.join("package.json"), manifest)
            .expect("write conditional package manifest");
        for entry in entries {
            std::fs::write(
                package_dir.join(entry),
                format!("interface {} {{}}", entry.replace('.', "_")),
            )
            .expect("write conditional package entry");
        }
        package_dir
    }

    fn resolver_with_limits(limits: Vue3ExternalTypeLoadLimits) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    #[test]
    fn reference_types_uses_effective_extended_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base_dir = dir.path().join("configs").join("base");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(project_dir.join("src")).expect("create project source dir");
        std::fs::create_dir_all(&base_dir).expect("create base config dir");
        let expected = write_type_package(&base_dir.join("types"), "referenced");
        let decoy = write_type_package(&project_dir.join("types"), "referenced");
        std::fs::write(
            base_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write base config");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{
                "extends":"../configs/base/tsconfig.json",
                "compilerOptions":{"types":[]}
            }"#,
        )
        .expect("write project config");
        let project = project_dir.join("src").join("Comp.vue");
        let containing = project_dir.join("src").join("ambient.d.ts");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "referenced",
                &resolver,
            ),
            Some(expected)
        );
        assert_ne!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "referenced",
                &Vue3TypeResolverContext::default(),
            ),
            Some(decoy)
        );
    }

    #[test]
    fn reference_types_applies_later_extends_and_direct_overrides() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let first_dir = dir.path().join("first");
        let second_dir = dir.path().join("second");
        std::fs::create_dir_all(project_dir.join("src")).expect("create project source dir");
        std::fs::create_dir_all(&first_dir).expect("create first config dir");
        std::fs::create_dir_all(&second_dir).expect("create second config dir");
        let _first = write_type_package(&first_dir.join("types"), "ordered");
        let second = write_type_package(&second_dir.join("types"), "ordered");
        let direct = write_type_package(&project_dir.join("types"), "direct");
        std::fs::write(
            first_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write first config");
        std::fs::write(
            second_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write second config");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{
                "extends":["../first/tsconfig.json","../second/tsconfig.json"],
                "compilerOptions":{"typeRoots":["./types"]}
            }"#,
        )
        .expect("write project config");
        let project = project_dir.join("src").join("Comp.vue");
        let containing = project_dir.join("src").join("ambient.d.ts");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "direct",
                &Vue3TypeResolverContext::default(),
            ),
            Some(direct)
        );

        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"extends":["../first/tsconfig.json","../second/tsconfig.json"]}"#,
        )
        .expect("replace project config");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "ordered",
                &Vue3TypeResolverContext::default(),
            ),
            Some(second)
        );
    }

    #[test]
    fn reference_types_use_the_nearest_project_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("packages").join("component");
        let source_dir = project_dir.join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source dir");
        let _outer = write_type_package(&dir.path().join("outer-types"), "nearest");
        let inner = write_type_package(&project_dir.join("inner-types"), "nearest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./outer-types"]}}"#,
        )
        .expect("write outer config");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./inner-types"]}}"#,
        )
        .expect("write nearest config");
        let filename = source_dir.join("Comp.vue");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "nearest",
                &Vue3TypeResolverContext::default(),
            ),
            Some(inner)
        );
    }

    #[test]
    fn reference_types_prefers_default_type_roots_then_uses_containing_file_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let containing_dir = dir.path().join("dependencies").join("consumer");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        std::fs::create_dir_all(&containing_dir).expect("create containing dir");
        let primary = write_type_package(
            &project_dir.join("node_modules").join("@types"),
            "preferred",
        );
        let _secondary_decoy =
            write_type_package(&containing_dir.join("node_modules"), "preferred");
        let secondary = write_type_package(&containing_dir.join("node_modules"), "secondary");
        let project = project_dir.join("Comp.vue");
        let containing = containing_dir.join("index.d.ts");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "preferred",
                &resolver,
            ),
            Some(primary)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "secondary",
                &resolver,
            ),
            Some(secondary)
        );
    }

    #[test]
    fn reference_types_empty_type_roots_still_use_containing_file_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let containing_dir = dir.path().join("dependency");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        std::fs::create_dir_all(&containing_dir).expect("create containing dir");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let expected = write_type_package(&containing_dir.join("node_modules"), "fallback");
        let project = project_dir.join("Comp.vue");
        let containing = containing_dir.join("ambient.d.ts");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "fallback",
                &Vue3TypeResolverContext::default(),
            ),
            Some(expected)
        );
    }

    #[test]
    fn reference_types_accept_backslash_relative_and_absolute_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let nested_dir = project_dir.join("types");
        std::fs::create_dir_all(&nested_dir).expect("create type directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let backslash = nested_dir.join("backslash.d.ts");
        let absolute = nested_dir.join("absolute.d.ts");
        std::fs::write(&backslash, "interface BackslashReference {}")
            .expect("write backslash declaration");
        std::fs::write(&absolute, "interface AbsoluteReference {}")
            .expect("write absolute declaration");
        let project = project_dir.join("Comp.vue");
        let containing = project_dir.join("ambient.d.ts");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                r#".\types\backslash"#,
                &resolver,
            ),
            Some(backslash)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                &absolute.to_string_lossy(),
                &resolver,
            ),
            Some(absolute)
        );
    }

    #[test]
    fn reference_types_explicit_roots_precede_relative_secondary_lookup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let source_dir = project_dir.join("src");
        let type_root = project_dir.join("types");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::create_dir_all(&type_root).expect("create type root");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./types"]}}"#,
        )
        .expect("write project config");
        let primary = type_root.join("local.d.ts");
        let secondary = source_dir.join("local.d.ts");
        std::fs::write(&primary, "interface PrimaryReference {}")
            .expect("write primary declaration");
        std::fs::write(&secondary, "interface SecondaryReference {}")
            .expect("write secondary declaration");
        let project = source_dir.join("Comp.vue");
        let containing = source_dir.join("ambient.d.ts");

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                r#".\local"#,
                &Vue3TypeResolverContext::default(),
            ),
            Some(primary)
        );
    }

    #[test]
    fn reference_types_secondary_lookup_is_declaration_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let node_modules = project_dir.join("node_modules");
        std::fs::create_dir_all(&node_modules).expect("create node_modules");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        let runtime = project_dir.join("runtime.ts");
        let declaration = project_dir.join("declaration.d.ts");
        std::fs::write(&runtime, "interface RuntimeOnly {}")
            .expect("write runtime source");
        std::fs::write(&declaration, "interface DeclarationOnly {}")
            .expect("write declaration source");
        let implicit_package = node_modules.join("implicit-runtime");
        std::fs::create_dir_all(&implicit_package).expect("create implicit runtime package");
        std::fs::write(
            implicit_package.join("index.ts"),
            "interface ImplicitRuntime {}",
        )
        .expect("write implicit runtime package entry");
        let explicit = write_type_package_with_entry(
            &node_modules,
            "explicit-runtime",
            "index.ts",
        );
        let exports_package = node_modules.join("exports-do-not-block-types");
        std::fs::create_dir_all(&exports_package).expect("create exports package");
        std::fs::write(
            exports_package.join("package.json"),
            r#"{"types":"index.d.ts","exports":{}}"#,
        )
        .expect("write exports package metadata");
        let exports_declaration = exports_package.join("index.d.ts");
        std::fs::write(
            &exports_declaration,
            "interface ExportsDoNotBlockTypes {}",
        )
        .expect("write exports package declaration");
        let main_package = node_modules.join("main-entry");
        let main_dist = main_package.join("dist");
        std::fs::create_dir_all(&main_dist).expect("create main package");
        std::fs::write(
            main_package.join("package.json"),
            r#"{"main":"dist/index.js"}"#,
        )
        .expect("write main package metadata");
        let main_declaration = main_dist.join("index.d.ts");
        std::fs::write(&main_declaration, "interface MainEntryTypes {}")
            .expect("write main package declaration");
        let dotted_decoy = node_modules.join("dotted.d.ts");
        let dotted_appended = node_modules.join("dotted.package.d.ts");
        let dotted_arbitrary = node_modules.join("dotted.d.package.ts");
        let hidden_appended = project_dir.join(".hidden.d.ts");
        let hidden_arbitrary = project_dir.join(".d.hidden.ts");
        std::fs::write(&dotted_decoy, "interface DottedDecoy {}")
            .expect("write dotted package decoy");
        std::fs::write(&dotted_appended, "interface DottedAppended {}")
            .expect("write appended dotted package declaration");
        std::fs::write(&dotted_arbitrary, "interface DottedArbitrary {}")
            .expect("write arbitrary-extension dotted package declaration");
        std::fs::write(&hidden_appended, "interface HiddenAppended {}")
            .expect("write appended hidden declaration");
        std::fs::write(&hidden_arbitrary, "interface HiddenArbitrary {}")
            .expect("write arbitrary-extension hidden declaration");
        let filename = project_dir.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "./runtime",
                &resolver,
            ),
            None
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "./declaration",
                &resolver,
            ),
            Some(declaration)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "implicit-runtime",
                &resolver,
            ),
            None
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "explicit-runtime",
                &resolver,
            ),
            Some(explicit)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "exports-do-not-block-types",
                &resolver,
            ),
            Some(exports_declaration)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "main-entry",
                &resolver,
            ),
            Some(main_declaration)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "dotted.package",
                &resolver,
            ),
            Some(dotted_arbitrary)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "./.hidden",
                &resolver,
            ),
            Some(hidden_arbitrary)
        );
    }

    #[test]
    fn reference_types_package_targets_honor_exact_generated_path_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let package_dir = project_dir.join("node_modules").join("limited-main");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&dist_dir).expect("create package directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":[]}}"#,
        )
        .expect("write project config");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"main":"dist/index.js"}"#,
        )
        .expect("write package metadata");
        let target = dist_dir.join("index.d.ts");
        std::fs::write(&target, "interface LimitedMainReference {}")
            .expect("write package declaration");
        let filename = project_dir.join("Comp.vue");
        let required = target.as_os_str().as_encoded_bytes().len();
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: required,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "limited-main",
                &exact,
            ),
            Some(target)
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: required - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "limited-main",
                &short,
            ),
            None
        );
        assert!(short.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn reference_types_match_typescript_scoped_package_locations() {
        let dir = tempfile::tempdir().expect("temp dir");
        let configured_project = dir.path().join("configured");
        let default_project = dir.path().join("default");
        std::fs::create_dir_all(&configured_project).expect("create configured project");
        std::fs::create_dir_all(&default_project).expect("create default project");
        let custom_root = configured_project.join("custom-types");
        let configured = write_type_package(&custom_root.join("@scope"), "package");
        let _invalid_mangled = write_type_package(&custom_root, "scope__invalid");
        std::fs::write(
            configured_project.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./custom-types"]}}"#,
        )
        .expect("write configured project config");
        let secondary = write_type_package(
            &default_project.join("node_modules").join("@types"),
            "scope__secondary",
        );
        std::fs::write(
            default_project.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[]}}"#,
        )
        .expect("write default project config");

        let resolver = Vue3TypeResolverContext {
            typescript_version: (5, 1, 0).into(),
            ..Default::default()
        };
        let configured_filename = configured_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &configured_filename.to_string_lossy(),
                &configured_filename.to_string_lossy(),
                "@scope/package",
                &resolver,
            ),
            Some(configured)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &configured_filename.to_string_lossy(),
                &configured_filename.to_string_lossy(),
                "@scope/invalid",
                &resolver,
            ),
            None
        );

        let default_filename = default_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &default_filename.to_string_lossy(),
                &default_filename.to_string_lossy(),
                "@scope/secondary",
                &resolver,
            ),
            Some(secondary)
        );
    }

    #[test]
    fn reference_types_mangle_default_scoped_packages_from_typescript_5_1() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let configured_project_dir = dir.path().join("configured-project");
        let containing_dir = dir.path().join("external");
        std::fs::create_dir_all(&project_dir).expect("create project");
        std::fs::create_dir_all(&configured_project_dir).expect("create configured project");
        std::fs::create_dir_all(&containing_dir).expect("create containing directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[]}}"#,
        )
        .expect("write project config");
        let expected = write_type_package(
            &project_dir.join("node_modules").join("@types"),
            "scope__versioned",
        );
        let subpath_dir = project_dir
            .join("node_modules")
            .join("@types")
            .join("scope__versioned")
            .join("subpath");
        std::fs::create_dir_all(&subpath_dir).expect("create scoped package subpath");
        let expected_subpath = subpath_dir.join("index.d.ts");
        std::fs::write(&expected_subpath, "interface ScopedSubpath {}")
            .expect("write scoped package subpath");
        std::fs::write(
            configured_project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./node_modules/@types"]}}"#,
        )
        .expect("write configured project config");
        let configured_expected = write_type_package(
            &configured_project_dir.join("node_modules").join("@types"),
            "scope__configured",
        );
        let project = project_dir.join("Comp.vue");
        let configured_project = configured_project_dir.join("Comp.vue");
        let containing = containing_dir.join("ambient.d.ts");
        let baseline = Vue3TypeResolverContext::default();
        let current = Vue3TypeResolverContext {
            typescript_version: (5, 1, 0).into(),
            ..Default::default()
        };

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/versioned",
                &baseline,
            ),
            None
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/versioned",
                &current,
            ),
            Some(expected)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/versioned/subpath",
                &current,
            ),
            Some(expected_subpath)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &configured_project.to_string_lossy(),
                &containing.to_string_lossy(),
                "@scope/configured",
                &current,
            ),
            Some(configured_expected)
        );
    }

    #[test]
    fn reference_types_flat_files_only_precede_explicit_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        let default_project = dir.path().join("default-project");
        let explicit_project = dir.path().join("explicit-project");
        let containing_dir = dir.path().join("external");
        std::fs::create_dir_all(&default_project).expect("create default project");
        std::fs::create_dir_all(&explicit_project).expect("create explicit project");
        std::fs::create_dir_all(&containing_dir).expect("create containing directory");

        let mut expected = Vec::new();
        for (project, config) in [
            (
                &default_project,
                r#"{"compilerOptions":{"types":[]}}"#,
            ),
            (
                &explicit_project,
                r#"{"compilerOptions":{"types":[],"typeRoots":["./node_modules/@types"]}}"#,
            ),
        ] {
            std::fs::write(project.join("tsconfig.json"), config).expect("write project config");
            let type_root = project.join("node_modules").join("@types");
            std::fs::create_dir_all(&type_root).expect("create type root");
            let flat = type_root.join("priority.d.ts");
            std::fs::write(&flat, "interface FlatPriority {}")
                .expect("write flat type declaration");
            let directory = write_type_package(&type_root, "priority");
            expected.push((flat, directory));
        }

        let containing = containing_dir.join("ambient.d.ts");
        let resolver = Vue3TypeResolverContext::default();
        let default_filename = default_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &default_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "priority",
                &resolver,
            ),
            Some(expected[0].1.clone())
        );
        let explicit_filename = explicit_project.join("Comp.vue");
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &explicit_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "priority",
                &resolver,
            ),
            Some(expected[1].0.clone())
        );
    }

    #[test]
    fn reference_types_accept_modern_declaration_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(&project_dir).expect("create project");
        let type_root = project_dir.join("types");
        let esm = write_type_package_with_entry(&type_root, "esm", "index.d.mts");
        let commonjs = write_type_package_with_entry(&type_root, "commonjs", "index.d.cts");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":[],"typeRoots":["./types"]}}"#,
        )
        .expect("write project config");
        let filename = project_dir.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "esm",
                &resolver,
            ),
            Some(esm)
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename.to_string_lossy(),
                &filename.to_string_lossy(),
                "commonjs",
                &resolver,
            ),
            Some(commonjs)
        );
    }

    #[test]
    fn reference_types_cache_is_project_scoped() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first_project = dir.path().join("first");
        let second_project = dir.path().join("second");
        let containing = dir.path().join("shared").join("ambient.d.ts");
        std::fs::create_dir_all(&first_project).expect("create first project");
        std::fs::create_dir_all(&second_project).expect("create second project");
        let first = write_type_package(&first_project.join("types"), "cached");
        let second = write_type_package(&second_project.join("types"), "cached");
        for project in [&first_project, &second_project] {
            std::fs::write(
                project.join("tsconfig.json"),
                r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
            )
            .expect("write project config");
        }
        let first_filename = first_project.join("Comp.vue");
        let second_filename = second_project.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive(
                &first_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "cached",
                &resolver,
            ),
            Some(first.clone())
        );
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &second_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "cached",
                &resolver,
            ),
            Some(second)
        );
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_cache_hits, 0);
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &first_filename.to_string_lossy(),
                &containing.to_string_lossy(),
                "cached",
                &resolver,
            ),
            Some(first)
        );
        assert_eq!(
            resolver.external_type_session.stats().resolution_cache_hits,
            1
        );
    }

    #[test]
    fn reference_types_resolution_modes_select_secondary_exports_and_isolate_cache() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let package_dir = project_dir
            .join("node_modules")
            .join("conditional-reference");
        std::fs::create_dir_all(&package_dir).expect("create package directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":[]}}"#,
        )
        .expect("write project config");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let import_entry = package_dir.join("import.d.mts");
        let require_entry = package_dir.join("require.d.cts");
        std::fs::write(&import_entry, "interface ImportReference {}")
            .expect("write import declaration");
        std::fs::write(&require_entry, "interface RequireReference {}")
            .expect("write require declaration");
        let filename = project_dir.join("Comp.vue");
        let filename = filename.to_string_lossy();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_reference_directive_with_mode(
                &filename,
                &filename,
                "conditional-reference",
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Some(import_entry.clone()),
        );
        assert_eq!(
            resolve_vue3_type_reference_directive_with_mode(
                &filename,
                &filename,
                "conditional-reference",
                Some(Vue3TypeResolutionMode::Require),
                &resolver,
            ),
            Some(require_entry.clone()),
        );
        assert_eq!(resolver.external_type_session.stats().resolution_cache_hits, 0);

        for (mode, expected) in [
            (Vue3TypeResolutionMode::Import, import_entry),
            (Vue3TypeResolutionMode::Require, require_entry),
        ] {
            assert_eq!(
                resolve_vue3_type_reference_directive_with_mode(
                    &filename,
                    &filename,
                    "conditional-reference",
                    Some(mode),
                    &resolver,
                ),
                Some(expected),
            );
        }
        assert_eq!(resolver.external_type_session.stats().resolution_cache_hits, 2);
        assert_eq!(
            resolve_vue3_type_reference_directive(
                &filename,
                &filename,
                "conditional-reference",
                &resolver,
            ),
            None,
        );
    }

    #[test]
    fn reference_types_resolution_modes_do_not_override_primary_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let package_dir = project_dir.join("types").join("primary-reference");
        std::fs::create_dir_all(&package_dir).expect("create package directory");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
        )
        .expect("write project config");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{
                "types": "./legacy.d.ts",
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let legacy_entry = package_dir.join("legacy.d.ts");
        std::fs::write(&legacy_entry, "interface LegacyPrimaryReference {}")
            .expect("write legacy declaration");
        std::fs::write(package_dir.join("import.d.mts"), "interface ImportDecoy {}")
            .expect("write import decoy");
        std::fs::write(package_dir.join("require.d.cts"), "interface RequireDecoy {}")
            .expect("write require decoy");
        let filename = project_dir.join("Comp.vue");
        let filename = filename.to_string_lossy();
        let resolver = Vue3TypeResolverContext::default();

        for mode in [
            Vue3TypeResolutionMode::Import,
            Vue3TypeResolutionMode::Require,
        ] {
            assert_eq!(
                resolve_vue3_type_reference_directive_with_mode(
                    &filename,
                    &filename,
                    "primary-reference",
                    Some(mode),
                    &resolver,
                ),
                Some(legacy_entry.clone()),
            );
        }
    }

    #[test]
    fn reference_types_conditional_exports_preserve_order_and_declaration_space() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let ordered = write_conditional_type_package(
            &node_modules,
            "ordered",
            r#"{
                "exports": {
                    ".": {
                        "default": "./default.d.ts",
                        "types": "./types.d.ts"
                    }
                }
            }"#,
            &["default.d.ts", "types.d.ts"],
        );
        let fallback = write_conditional_type_package(
            &node_modules,
            "fallback",
            r#"{
                "exports": {
                    ".": {
                        "types": "./missing.d.ts",
                        "import": "./import.d.mts",
                        "require": "./require.d.cts",
                        "default": "./default.d.ts"
                    }
                }
            }"#,
            &["import.d.mts", "require.d.cts", "default.d.ts"],
        );
        let declaration_only = write_conditional_type_package(
            &node_modules,
            "declaration-only",
            r#"{
                "exports": {
                    ".": {
                        "import": "./runtime.ts",
                        "default": "./fallback.d.ts"
                    }
                }
            }"#,
            &["runtime.ts", "fallback.d.ts"],
        );
        let require_only = write_conditional_type_package(
            &node_modules,
            "require-only",
            r#"{"exports":{".":{"require":"./require.d.cts"}}}"#,
            &["require.d.cts"],
        );
        let resolver = Vue3TypeResolverContext::default();

        for mode in [
            Vue3TypeResolutionMode::Import,
            Vue3TypeResolutionMode::Require,
        ] {
            assert_eq!(
                resolve_vue3_package_json_type_reference_entry(
                    &ordered,
                    None,
                    Some(mode),
                    &resolver,
                ),
                Vue3PackageJsonTypeResolution::Resolved(ordered.join("default.d.ts")),
            );
        }
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &fallback,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Resolved(fallback.join("import.d.mts")),
        );
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &fallback,
                None,
                Some(Vue3TypeResolutionMode::Require),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Resolved(fallback.join("require.d.cts")),
        );
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &declaration_only,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Resolved(declaration_only.join("fallback.d.ts")),
        );
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &require_only,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &resolver,
            ),
            Vue3PackageJsonTypeResolution::Blocked,
        );
    }

    #[test]
    fn reference_types_conditional_export_fanout_is_bounded() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = write_conditional_type_package(
            dir.path(),
            "bounded",
            r#"{
                "exports": {
                    ".": {
                        "types": "./missing.d.ts",
                        "import": "./hit.d.mts"
                    }
                }
            }"#,
            &["hit.d.mts"],
        );
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &package,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &exact,
            ),
            Vue3PackageJsonTypeResolution::Resolved(package.join("hit.d.mts")),
        );
        assert_eq!(exact.external_type_session.stats().metadata_fanout_entries, 2);

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            resolve_vue3_package_json_type_reference_entry(
                &package,
                None,
                Some(Vue3TypeResolutionMode::Import),
                &short,
            ),
            Vue3PackageJsonTypeResolution::Blocked,
        );
        assert_eq!(short.external_type_session.stats().metadata_fanout_entries, 1);
        assert!(short.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn reference_types_metadata_exhaustion_prevents_secondary_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project_dir = dir.path().join("project");
        let containing_dir = project_dir.join("dependencies");
        std::fs::create_dir_all(&containing_dir).expect("create containing dir");
        let _secondary = write_type_package(&containing_dir.join("node_modules"), "blocked");
        std::fs::write(
            project_dir.join("tsconfig.json"),
            r#"{"extends":"./base.json"}"#,
        )
        .expect("write project config");
        std::fs::write(project_dir.join("base.json"), "{}").expect("write base config");
        let project = project_dir.join("Comp.vue");
        let containing = containing_dir.join("ambient.d.ts");
        let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(resolve_vue3_type_reference_directive(
            &project.to_string_lossy(),
            &containing.to_string_lossy(),
            "blocked",
            &resolver,
        )
        .is_none());
        assert!(resolver.external_type_session.metadata_is_blocked());
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .metadata_fanout_entries,
            0
        );
    }
}
