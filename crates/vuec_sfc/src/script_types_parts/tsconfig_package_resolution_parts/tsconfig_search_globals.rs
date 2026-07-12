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

const VUE3_TSCONFIG_INCLUDE_MAX_DEPTH: usize = 64;
const VUE3_TSCONFIG_INCLUDE_MAX_ENTRIES: usize = 65_536;
const VUE3_TSCONFIG_INCLUDE_MAX_FILES: usize = 16_384;

#[derive(Debug)]
pub(crate) struct Vue3TsconfigIncludeScanBudget {
    max_depth: usize,
    remaining_entries: usize,
    remaining_files: usize,
}

impl Vue3TsconfigIncludeScanBudget {
    pub(crate) fn new(max_depth: usize, max_entries: usize, max_files: usize) -> Self {
        Self {
            max_depth,
            remaining_entries: max_entries,
            remaining_files: max_files,
        }
    }

    fn claim_file(&mut self) -> bool {
        if self.remaining_files == 0 {
            return false;
        }
        self.remaining_files -= 1;
        true
    }
}

impl Default for Vue3TsconfigIncludeScanBudget {
    fn default() -> Self {
        Self::new(
            VUE3_TSCONFIG_INCLUDE_MAX_DEPTH,
            VUE3_TSCONFIG_INCLUDE_MAX_ENTRIES,
            VUE3_TSCONFIG_INCLUDE_MAX_FILES,
        )
    }
}

pub(crate) fn resolve_vue3_tsconfig_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    for config_path in vue3_tsconfig_search_paths(filename) {
        let config_dir = config_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let mut seen = BTreeSet::new();
        let mappings =
            vue3_tsconfig_path_mappings_from_config(&config_path, &config_dir, &mut seen);
        if let Some(resolved) =
            resolve_vue3_tsconfig_path_mappings(&mappings, source, type_resolver)
        {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn vue3_tsconfig_search_paths(filename: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut current = Path::new(filename).parent();
    while let Some(dir) = current {
        let candidate = normalize_path_components(dir.join("tsconfig.json"));
        if candidate.is_file() {
            paths.push(candidate);
        }
        current = dir.parent();
    }
    paths
}

pub(crate) fn vue3_tsconfig_path_mappings_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    seen: &mut BTreeSet<String>,
) -> Vec<Vue3TsconfigPathMapping> {
    let normalized = normalize_path_string(config_path);
    if !seen.insert(normalized) {
        return Vec::new();
    }
    let Ok(source) = std::fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Some(value) = vue3_parse_tsconfig_jsonc(&source) else {
        return Vec::new();
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let mut mappings = Vec::new();
    for extended in vue3_tsconfig_extends_paths(&value, config_dir) {
        mappings.extend(vue3_tsconfig_path_mappings_from_config(
            &extended,
            template_config_dir,
            seen,
        ));
    }
    let direct = vue3_tsconfig_direct_path_mappings(&value, config_dir, template_config_dir);
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
            seen,
        ));
    }
    mappings
}

pub(crate) fn vue3_tsconfig_global_type_files(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen_configs = BTreeSet::new();
    let mut seen_files = BTreeSet::new();
    for config_path in vue3_tsconfig_search_paths(filename) {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &config_path,
            config_dir,
            &mut seen_configs,
            &mut seen_files,
            &mut files,
            type_resolver,
        );
    }
    files
}

pub(crate) fn vue3_tsconfig_global_type_files_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    seen_configs: &mut BTreeSet<String>,
    seen_files: &mut BTreeSet<String>,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) {
    let normalized = normalize_path_string(config_path);
    if !seen_configs.insert(normalized) {
        return;
    }
    let Ok(source) = std::fs::read_to_string(config_path) else {
        return;
    };
    let Some(value) = vue3_parse_tsconfig_jsonc(&source) else {
        return;
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    for extended in vue3_tsconfig_extends_paths(&value, config_dir) {
        vue3_tsconfig_global_type_files_from_config(
            &extended,
            template_config_dir,
            seen_configs,
            seen_files,
            files,
            type_resolver,
        );
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
            seen_configs,
            seen_files,
            files,
            type_resolver,
        );
    }
}

pub(crate) fn vue3_tsconfig_direct_global_type_files(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut include_scan_budget = Vue3TsconfigIncludeScanBudget::default();
    for target in vue3_tsconfig_string_array(value.get("files")) {
        let path = vue3_tsconfig_target_path(config_dir, template_config_dir, &target, "");
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            files.push(path);
        }
    }
    for target in vue3_tsconfig_string_array(value.get("include")) {
        files.extend(vue3_tsconfig_include_global_type_files(
            config_dir,
            template_config_dir,
            &target,
            &mut include_scan_budget,
        ));
    }
    files.extend(vue3_tsconfig_compiler_option_global_type_files(
        value,
        config_dir,
        template_config_dir,
        type_resolver,
    ));
    files
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
    let has_configured_type_roots =
        compiler_options.is_some_and(|options| options.get("typeRoots").is_some());
    let configured_type_roots =
        vue3_tsconfig_string_array(compiler_options.and_then(|options| options.get("typeRoots")))
            .into_iter()
            .filter_map(|target| {
                let path = vue3_tsconfig_target_path(config_dir, template_config_dir, &target, "");
                path.is_dir().then_some(path)
            })
            .collect::<Vec<_>>();
    let type_roots = if has_configured_type_roots {
        configured_type_roots
    } else {
        vue3_tsconfig_default_type_roots(config_dir)
    };
    if compiler_options.is_some_and(|options| options.get("types").is_some()) {
        let types =
            vue3_tsconfig_string_array(compiler_options.and_then(|options| options.get("types")));
        return types
            .into_iter()
            .flat_map(|type_name| {
                vue3_tsconfig_named_type_global_type_files(&type_roots, &type_name, type_resolver)
            })
            .collect();
    }
    type_roots
        .into_iter()
        .flat_map(|type_root| {
            vue3_tsconfig_all_type_root_global_type_files(&type_root, type_resolver)
        })
        .collect()
}

pub(crate) fn vue3_tsconfig_default_type_roots(config_dir: &Path) -> Vec<PathBuf> {
    vue3_node_modules_search_paths_from_dir(config_dir)
        .into_iter()
        .map(|node_modules| normalize_path_components(node_modules.join("@types")))
        .filter(|path| path.is_dir())
        .collect()
}

pub(crate) fn vue3_tsconfig_named_type_global_type_files(
    type_roots: &[PathBuf],
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_type_name_is_safe(type_name) {
        return Vec::new();
    }
    type_roots
        .iter()
        .flat_map(|type_root| vue3_tsconfig_type_name_package_dirs(type_root, type_name))
        .filter_map(|package_dir| {
            vue3_tsconfig_type_package_global_type_file(&package_dir, type_resolver)
        })
        .collect()
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
    let Ok(entries) = std::fs::read_dir(type_root) else {
        return Vec::new();
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
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
    }
    files
}

pub(crate) fn vue3_tsconfig_all_scoped_type_root_global_type_files(
    scope_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(scope_dir) else {
        return Vec::new();
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    entries
        .into_iter()
        .filter(|entry| entry.is_dir())
        .filter(|entry| {
            entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.is_empty() && !name.starts_with('.'))
        })
        .filter_map(|entry| vue3_tsconfig_type_package_global_type_file(&entry, type_resolver))
        .collect()
}

pub(crate) fn vue3_tsconfig_type_package_global_type_file(
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let path = resolve_vue3_package_type_entry(package_dir, None, type_resolver)?;
    vue3_tsconfig_global_type_file_is_supported(&path).then_some(path)
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
    scan_budget: &mut Vue3TsconfigIncludeScanBudget,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_include_can_match_global_type_files(target) {
        return Vec::new();
    }
    if !target.contains('*') && !target.contains('?') {
        let path = vue3_tsconfig_target_path(config_dir, template_config_dir, target, "");
        if vue3_tsconfig_global_type_file_is_supported(&path) && scan_budget.claim_file() {
            return vec![path];
        }
        if path.is_dir() {
            let mut files = Vec::new();
            vue3_collect_global_type_files_from_dir(&path, &mut files, scan_budget);
            return files;
        }
        return Vec::new();
    }
    let Some(root) = vue3_tsconfig_include_root_path(config_dir, template_config_dir, target)
    else {
        return Vec::new();
    };
    let pattern = vue3_tsconfig_include_pattern(config_dir, template_config_dir, target);
    let mut files = Vec::new();
    vue3_collect_global_type_files_from_dir(&root, &mut files, scan_budget);
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
) -> String {
    let target = target.replace(
        "${configDir}",
        normalize_path_string(template_config_dir).as_str(),
    );
    let path = Path::new(&target);
    if path.is_absolute() {
        normalize_path_string(&normalize_path_components(PathBuf::from(target)))
    } else {
        normalize_path_string(&normalize_path_components(config_dir.join(target)))
    }
}

pub(crate) fn vue3_tsconfig_include_root_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
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
    let path = vue3_tsconfig_target_path(config_dir, template_config_dir, &root, "");
    path.is_dir().then_some(path)
}

pub(crate) fn vue3_collect_global_type_files_from_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    scan_budget: &mut Vue3TsconfigIncludeScanBudget,
) {
    let mut seen_dirs = BTreeSet::new();
    vue3_collect_global_type_files_from_dir_inner(
        dir,
        files,
        scan_budget,
        &mut seen_dirs,
        0,
    );
}

fn vue3_collect_global_type_files_from_dir_inner(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    scan_budget: &mut Vue3TsconfigIncludeScanBudget,
    seen_dirs: &mut BTreeSet<String>,
    depth: usize,
) {
    if scan_budget.remaining_entries == 0 || scan_budget.remaining_files == 0 {
        return;
    }
    let canonical_dir = std::fs::canonicalize(dir)
        .unwrap_or_else(|_| normalize_path_components(dir.to_path_buf()));
    if !seen_dirs.insert(normalize_path_string(&canonical_dir)) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let entries = entries
        .take(scan_budget.remaining_entries)
        .collect::<Vec<_>>();
    scan_budget.remaining_entries = scan_budget.remaining_entries.saturating_sub(entries.len());
    let mut entries = entries
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some((entry.path(), entry.file_type().ok()?))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (path, file_type) in entries {
        if scan_budget.remaining_files == 0 {
            break;
        }
        if file_type.is_symlink() {
            continue;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if file_type.is_dir() {
            if depth < scan_budget.max_depth {
                vue3_collect_global_type_files_from_dir_inner(
                    &path,
                    files,
                    scan_budget,
                    seen_dirs,
                    depth + 1,
                );
            }
        } else if file_type.is_file()
            && vue3_tsconfig_global_type_file_is_supported(&path)
            && scan_budget.claim_file()
        {
            files.push(normalize_path_components(path));
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
