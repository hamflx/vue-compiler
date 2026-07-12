#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigPathMapping {
    pub(crate) pattern: String,
    pub(crate) targets: Vec<String>,
    pub(crate) target_base_dir: PathBuf,
    pub(crate) template_config_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigPathMatch<'a> {
    pub(crate) mapping: &'a Vue3TsconfigPathMapping,
    pub(crate) capture: String,
    pub(crate) score: usize,
    pub(crate) order: usize,
}

#[derive(Debug, Default)]
struct Vue3TsconfigGraphTraversal {
    seen_states: BTreeSet<(PathBuf, PathBuf, PathBuf)>,
    active_identities: BTreeSet<PathBuf>,
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
    let identity = vue3_external_type_path_identity(config_path);
    if traversal.active_identities.contains(&identity) {
        return None;
    }
    let state_key = (
        identity.clone(),
        vue3_external_type_lexical_path(
            config_path.parent().unwrap_or_else(|| Path::new("")),
        ),
        vue3_external_type_lexical_path(template_config_dir),
    );
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

pub(crate) fn resolve_vue3_tsconfig_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let mut traversal = Vue3TsconfigGraphTraversal::default();
    for config_path in vue3_tsconfig_search_paths(filename, type_resolver) {
        let config_dir = config_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let mappings = vue3_tsconfig_path_mappings_from_config(
            &config_path,
            &config_dir,
            &mut traversal,
            0,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        let resolved = resolve_vue3_tsconfig_path_mappings(&mappings, source, type_resolver);
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if let Some(resolved) = resolved {
            return Some(resolved);
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
    .filter(|candidate| candidate.is_file())
}

fn vue3_tsconfig_path_mappings_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigGraphTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<Vue3TsconfigPathMapping> {
    let Some(identity) = vue3_tsconfig_graph_enter(
        config_path,
        template_config_dir,
        depth,
        traversal,
        type_resolver,
    ) else {
        return Vec::new();
    };
    let mappings = (|| {
        let value = type_resolver
            .external_type_session
            .tsconfig_from_path(config_path)?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        let mut mappings = Vec::new();
        for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
            mappings.extend(vue3_tsconfig_path_mappings_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            ));
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Some(Vec::new());
        }
        let direct = vue3_tsconfig_direct_path_mappings(
            &value,
            config_dir,
            template_config_dir,
            type_resolver,
        );
        if !direct.is_empty() {
            let direct_patterns = direct
                .iter()
                .map(|mapping| mapping.pattern.as_str())
                .collect::<BTreeSet<_>>();
            mappings.retain(|mapping| !direct_patterns.contains(mapping.pattern.as_str()));
            mappings.extend(direct);
        }
        for reference in vue3_tsconfig_reference_paths(&value, config_dir) {
            let reference_dir = reference.parent().unwrap_or_else(|| Path::new(""));
            mappings.extend(vue3_tsconfig_path_mappings_from_config(
                &reference,
                reference_dir,
                traversal,
                depth + 1,
                type_resolver,
            ));
        }
        Some(mappings)
    })()
    .unwrap_or_default();
    traversal.active_identities.remove(&identity);
    mappings
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
    for reference in vue3_tsconfig_reference_paths(&value, config_dir) {
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
        let Some(path) = vue3_tsconfig_target_path(
            config_dir,
            template_config_dir,
            &target,
            "",
            type_resolver,
        ) else {
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
    for target in vue3_tsconfig_string_array(value.get("include")) {
        files.extend(vue3_tsconfig_include_global_type_files(
            config_dir,
            template_config_dir,
            &target,
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
        let Some(path) = vue3_tsconfig_target_path(
            config_dir,
            template_config_dir,
            &target,
            "",
            type_resolver,
        ) else {
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
            .is_some_and(|name| name.ends_with(".d.ts"))
}

pub(crate) fn vue3_tsconfig_include_global_type_files(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
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
        let Some(path) = vue3_tsconfig_target_path(
            config_dir,
            template_config_dir,
            target,
            "",
            type_resolver,
        ) else {
            return Vec::new();
        };
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            if !type_resolver
                .external_type_session
                .claim_tsconfig_discovery_file()
            {
                return Vec::new();
            }
            return vec![path];
        }
        if path.is_dir() {
            let mut files = Vec::new();
            vue3_collect_global_type_files_from_dir(&path, &mut files, type_resolver);
            return files;
        }
        return Vec::new();
    }
    let Some(root) = vue3_tsconfig_include_root_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    ) else {
        return Vec::new();
    };
    let Some(pattern) =
        vue3_tsconfig_include_pattern(config_dir, template_config_dir, target, type_resolver)
    else {
        return Vec::new();
    };
    let mut files = Vec::new();
    vue3_collect_global_type_files_from_dir(&root, &mut files, type_resolver);
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vec::new();
    }
    files
        .into_iter()
        .filter(|file| vue3_tsconfig_glob_matches(&pattern, &normalize_path_string(file)))
        .collect()
}

pub(crate) fn vue3_tsconfig_include_can_match_global_type_files(target: &str) -> bool {
    let file_pattern = target.rsplit('/').next().unwrap_or(target);
    if !file_pattern.contains('.') {
        return true;
    }
    file_pattern.ends_with(".d.ts") || file_pattern.ends_with(".ts")
}

pub(crate) fn vue3_tsconfig_include_pattern(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    let template_config_dir = normalize_path_string(template_config_dir);
    let target = type_resolver.external_type_session.replace_metadata_path_pattern(
        target,
        "${configDir}",
        &template_config_dir,
    )?;
    let path = Path::new(&target);
    if path.is_absolute() {
        Some(normalize_path_string(&normalize_path_components(
            PathBuf::from(target),
        )))
    } else {
        Some(normalize_path_string(&normalize_path_components(
            config_dir.join(target),
        )))
    }
}

pub(crate) fn vue3_tsconfig_include_root_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if target.is_empty() || target.contains('\\') || target.contains(':') {
        return None;
    }
    let root = target
        .split('/')
        .take_while(|segment| !segment.contains('*') && !segment.contains('?'))
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>();
    if root.contains(&"..") {
        return None;
    }
    let root = if root.is_empty() {
        ".".to_string()
    } else {
        root.join("/")
    };
    let path = vue3_tsconfig_target_path(
        config_dir,
        template_config_dir,
        &root,
        "",
        type_resolver,
    )?;
    path.is_dir().then_some(path)
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
    let canonical_dir = std::fs::canonicalize(dir)
        .unwrap_or_else(|_| normalize_path_components(dir.to_path_buf()));
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
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
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
        } else if file_type.is_file()
            && vue3_tsconfig_global_type_file_is_supported(&path)
        {
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

pub(crate) fn vue3_tsconfig_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern_parts = pattern.split('/').collect::<Vec<_>>();
    let path_parts = path.split('/').collect::<Vec<_>>();
    vue3_tsconfig_glob_parts_match(&pattern_parts, &path_parts)
}

pub(crate) fn vue3_tsconfig_glob_parts_match(pattern: &[&str], path: &[&str]) -> bool {
    let mut previous = vec![false; path.len() + 1];
    let mut current = vec![false; path.len() + 1];
    previous[0] = true;
    for pattern_part in pattern {
        current.fill(false);
        if *pattern_part == "**" {
            current[0] = previous[0];
            for path_index in 1..=path.len() {
                current[path_index] = previous[path_index] || current[path_index - 1];
            }
        } else {
            for path_index in 1..=path.len() {
                current[path_index] = previous[path_index - 1]
                    && vue3_tsconfig_glob_segment_match(
                        pattern_part,
                        path[path_index - 1],
                    );
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[path.len()]
}

pub(crate) fn vue3_tsconfig_glob_segment_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    let mut previous = vec![false; text.len() + 1];
    previous[0] = true;
    for pattern_ch in pattern {
        let mut current = vec![false; text.len() + 1];
        if pattern_ch == '*' {
            current[0] = previous[0];
            for index in 1..=text.len() {
                current[index] = previous[index] || current[index - 1];
            }
        } else {
            for index in 1..=text.len() {
                current[index] =
                    previous[index - 1] && (pattern_ch == '?' || pattern_ch == text[index - 1]);
            }
        }
        previous = current;
    }
    previous[text.len()]
}
