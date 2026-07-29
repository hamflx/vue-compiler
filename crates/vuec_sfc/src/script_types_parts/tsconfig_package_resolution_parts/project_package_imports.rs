#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue3TsconfigEmitPathOptions {
    root_dir: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    declaration_dir: Option<PathBuf>,
    composite: Option<bool>,
}

impl Vue3TsconfigEmitPathOptions {
    fn overlay(&mut self, other: Self) {
        if other.root_dir.is_some() {
            self.root_dir = other.root_dir;
        }
        if other.out_dir.is_some() {
            self.out_dir = other.out_dir;
        }
        if other.declaration_dir.is_some() {
            self.declaration_dir = other.declaration_dir;
        }
        if other.composite.is_some() {
            self.composite = other.composite;
        }
    }
}

#[derive(Debug, Default)]
struct Vue3TsconfigEmitPathTraversal {
    active_identities: BTreeSet<PathBuf>,
    cached_options: BTreeMap<Vue3TsconfigGraphStateKey, Vue3TsconfigEmitPathOptions>,
}

fn vue3_tsconfig_emit_path_options(
    filename: &str,
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<(PathBuf, Vue3TsconfigEmitPathOptions)> {
    let config_path = vue3_tsconfig_search_paths(filename, type_resolver).next()?;
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let config_path = normalize_path_components(config_path);
    let package_dir = normalize_path_components(package_dir.to_path_buf());
    if !config_path.starts_with(&package_dir) {
        return None;
    }
    let template_config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let mut traversal = Vue3TsconfigEmitPathTraversal::default();
    let options = vue3_tsconfig_emit_path_options_from_config(
        &config_path,
        template_config_dir,
        &mut traversal,
        0,
        type_resolver,
    )?;
    Some((config_path, options))
}

fn vue3_tsconfig_emit_path_options_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    traversal: &mut Vue3TsconfigEmitPathTraversal,
    depth: usize,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigEmitPathOptions> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    let state_key = vue3_tsconfig_graph_state_key(config_path, template_config_dir);
    if let Some(cached) = traversal.cached_options.get(&state_key) {
        return Some(cached.clone());
    }
    let identity = state_key.0.clone();
    if traversal.active_identities.contains(&identity) {
        return Some(Vue3TsconfigEmitPathOptions::default());
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
        let mut effective = Vue3TsconfigEmitPathOptions::default();
        for extended in vue3_tsconfig_extends_paths(&value, config_dir, type_resolver) {
            let extended_options = vue3_tsconfig_emit_path_options_from_config(
                &extended,
                template_config_dir,
                traversal,
                depth + 1,
                type_resolver,
            )?;
            effective.overlay(extended_options);
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        effective.overlay(vue3_tsconfig_direct_emit_path_options(
            &value,
            config_dir,
            template_config_dir,
            type_resolver,
        )?);
        Some(effective)
    })();
    traversal.active_identities.remove(&identity);
    if let Some(options) = &resolved {
        traversal
            .cached_options
            .insert(state_key, options.clone());
    }
    resolved
}

fn vue3_tsconfig_direct_emit_path_options(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TsconfigEmitPathOptions> {
    let Some(compiler_options) = value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
    else {
        return Some(Vue3TsconfigEmitPathOptions::default());
    };
    let mut options = Vue3TsconfigEmitPathOptions::default();
    for (name, destination) in [
        ("rootDir", &mut options.root_dir),
        ("outDir", &mut options.out_dir),
        ("declarationDir", &mut options.declaration_dir),
    ] {
        let Some(target) = compiler_options
            .get(name)
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        *destination = vue3_tsconfig_compiler_option_path(
            config_dir,
            template_config_dir,
            target,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
    }
    options.composite = compiler_options
        .get("composite")
        .and_then(serde_json::Value::as_bool);
    Some(options)
}

fn vue3_tsconfig_compiler_option_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if target.is_empty() {
        return None;
    }
    vue3_tsconfig_target_path(
        config_dir,
        template_config_dir,
        target,
        type_resolver,
    )
}

fn resolve_vue3_project_package_input_target_with_mode(
    importer: &Path,
    package_dir: &Path,
    target: &str,
    config_path: &Path,
    options: &Vue3TsconfigEmitPathOptions,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_project_package_input_target_with_pass(
        importer,
        package_dir,
        target,
        config_path,
        options,
        resolution_mode,
        Vue3ProjectPackageInputPass::All,
        type_resolver,
    )
}

fn resolve_vue3_project_package_input_target_for_phase_with_mode(
    importer: &Path,
    package_dir: &Path,
    target: &str,
    config_path: &Path,
    options: &Vue3TsconfigEmitPathOptions,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_project_package_input_target_with_pass(
        importer,
        package_dir,
        target,
        config_path,
        options,
        resolution_mode,
        Vue3ProjectPackageInputPass::Phase(phase),
        type_resolver,
    )
}

#[derive(Clone, Copy)]
enum Vue3ProjectPackageInputPass {
    All,
    Phase(Vue3PackageResolutionPhase),
}

fn resolve_vue3_project_package_input_target_with_pass(
    importer: &Path,
    package_dir: &Path,
    target: &str,
    config_path: &Path,
    options: &Vue3TsconfigEmitPathOptions,
    resolution_mode: Vue3TypeResolutionMode,
    pass: Vue3ProjectPackageInputPass,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if options.out_dir.is_none() && options.declaration_dir.is_none() {
        return None;
    }
    let relative_target = target.strip_prefix("./")?;
    let final_path = vue3_materialized_project_package_path(
        package_dir,
        Path::new(relative_target),
        type_resolver,
    )?;
    let source_roots = vue3_project_package_source_root_guesses(
        importer,
        package_dir,
        config_path,
        options,
        type_resolver,
    )?;
    let mut output_dirs = Vec::with_capacity(2);
    if let Some(declaration_dir) = &options.declaration_dir {
        output_dirs.push(declaration_dir);
    }
    if let Some(out_dir) = &options.out_dir {
        if options.declaration_dir.as_ref() != Some(out_dir) {
            output_dirs.push(out_dir);
        }
    }
    for source_root in source_roots {
        for output_dir in output_dirs.iter().copied() {
            let path_fragment = match vue3_path_relative_to(
                &final_path,
                output_dir,
                type_resolver,
            ) {
                Some(path_fragment) => path_fragment,
                None if type_resolver.external_type_session.metadata_is_blocked() => return None,
                None => continue,
            };
            if path_fragment.as_os_str().is_empty() {
                continue;
            }
            let possible_input = vue3_materialized_project_package_path(
                &source_root,
                &path_fragment,
                type_resolver,
            )?;
            let Some((stem, input_extensions)) =
                vue3_possible_project_input_path_parts(&possible_input)
            else {
                continue;
            };
            for extension in input_extensions {
                let Some(candidate_phase) =
                    vue3_project_package_input_extension_phase(extension)
                else {
                    continue;
                };
                if matches!(pass, Vue3ProjectPackageInputPass::Phase(phase) if phase != candidate_phase)
                {
                    continue;
                }
                let candidate = vue3_materialized_project_package_input_candidate(
                    &possible_input,
                    stem,
                    extension,
                    type_resolver,
                )?;
                if !type_resolver
                    .external_type_session
                    .metadata_path_is_within_limit(&normalize_path_string(&candidate))
                {
                    return None;
                }
                if !type_resolver
                    .external_type_session
                    .metadata_path_is_file(&candidate)?
                {
                    continue;
                }
                return match candidate_phase {
                    Vue3PackageResolutionPhase::Types => {
                        resolve_vue3_metadata_package_map_type_target_path_with_mode(
                            &candidate,
                            resolution_mode,
                            type_resolver,
                        )
                    }
                    Vue3PackageResolutionPhase::JavaScript => {
                        resolve_vue3_metadata_legacy_package_javascript_field_path(
                            &candidate,
                            Vue3PackageTargetPathPolicy::RequireExplicitFileName,
                            type_resolver,
                        )
                    }
                };
            }
        }
    }
    None
}

fn vue3_project_package_input_extension_phase(
    extension: &str,
) -> Option<Vue3PackageResolutionPhase> {
    match extension {
        ".ts" | ".tsx" | ".mts" | ".cts" => Some(Vue3PackageResolutionPhase::Types),
        ".js" | ".jsx" | ".mjs" | ".cjs" => {
            Some(Vue3PackageResolutionPhase::JavaScript)
        }
        _ => None,
    }
}

fn resolve_vue3_package_relative_target_with_project_input(
    importer: &Path,
    package_dir: &Path,
    target: &str,
    emit_path_options: Option<&(PathBuf, Vue3TsconfigEmitPathOptions)>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    target.strip_prefix("./")?;
    let input = emit_path_options.and_then(|(config_path, options)| {
        resolve_vue3_project_package_input_target_with_mode(
            importer,
            package_dir,
            target,
            config_path,
            options,
            resolution_mode,
            type_resolver,
        )
    });
    if input.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
        return input;
    }
    for phase in [
        Vue3PackageResolutionPhase::Types,
        Vue3PackageResolutionPhase::JavaScript,
    ] {
        let resolved = vue3_package_export_path_for_phase_with_mode(
            package_dir,
            target,
            resolution_mode,
            phase,
            type_resolver,
        );
        if resolved.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return resolved;
        }
    }
    None
}

fn resolve_vue3_package_relative_target_with_project_input_for_phase(
    importer: &Path,
    package_dir: &Path,
    target: &str,
    emit_path_options: Option<&(PathBuf, Vue3TsconfigEmitPathOptions)>,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    target.strip_prefix("./")?;
    let input = emit_path_options.and_then(|(config_path, options)| {
        resolve_vue3_project_package_input_target_for_phase_with_mode(
            importer,
            package_dir,
            target,
            config_path,
            options,
            resolution_mode,
            phase,
            type_resolver,
        )
    });
    if input.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
        input
    } else {
        vue3_package_export_path_for_phase_with_mode(
            package_dir,
            target,
            resolution_mode,
            phase,
            type_resolver,
        )
    }
}

fn vue3_project_package_source_root_guesses(
    importer: &Path,
    package_dir: &Path,
    config_path: &Path,
    options: &Vue3TsconfigEmitPathOptions,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vec<PathBuf>> {
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let mut roots = Vec::new();
    if let Some(root_dir) = &options.root_dir {
        if !vue3_push_project_package_source_root(&mut roots, root_dir, type_resolver) {
            return None;
        }
    } else if options.composite == Some(true)
        || type_resolver.typescript_version >= (6, 0, 0).into()
    {
        if !vue3_push_project_package_source_root(&mut roots, config_dir, type_resolver) {
            return None;
        }
    } else {
        let importer_dir = importer.parent().unwrap_or_else(|| Path::new(""));
        let common = importer_dir
            .ancestors()
            .find(|ancestor| package_dir.starts_with(ancestor))?;
        for root in common.ancestors() {
            if !vue3_push_project_package_source_root(&mut roots, root, type_resolver) {
                return None;
            }
        }
        roots.reverse();
    }
    Some(roots)
}

fn vue3_push_project_package_source_root(
    roots: &mut Vec<PathBuf>,
    root: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    if !type_resolver
        .external_type_session
        .claim_metadata_fanout_entry()
    {
        return false;
    }
    let path_bytes = root.as_os_str().as_encoded_bytes().len();
    if !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver) {
        return false;
    }
    let root = normalize_path_components(root.to_path_buf());
    debug_assert!(root.as_os_str().as_encoded_bytes().len() <= path_bytes);
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&root))
    {
        return false;
    }
    roots.push(root);
    true
}

fn vue3_materialized_project_package_path(
    base: &Path,
    suffix: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let path_bytes = base
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .saturating_add(usize::from(
            !base.as_os_str().is_empty() && !suffix.as_os_str().is_empty(),
        ))
        .saturating_add(suffix.as_os_str().as_encoded_bytes().len());
    if !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver) {
        return None;
    }
    let path = normalize_path_components(base.join(suffix));
    debug_assert!(path.as_os_str().as_encoded_bytes().len() <= path_bytes);
    type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&path))
        .then_some(path)
}

fn vue3_path_relative_to(
    path: &Path,
    base: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let mut path_components = path.components();
    for base_component in base.components() {
        let path_component = path_components.next()?;
        #[cfg(windows)]
        let matches = path_component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&base_component.as_os_str().to_string_lossy());
        #[cfg(not(windows))]
        let matches = path_component == base_component;
        if !matches {
            return None;
        }
    }
    let mut path_bytes = 0usize;
    let mut has_component = false;
    for component in path_components.clone() {
        path_bytes = path_bytes
            .saturating_add(usize::from(has_component))
            .saturating_add(component.as_os_str().as_encoded_bytes().len());
        has_component = true;
    }
    if !has_component {
        return Some(PathBuf::new());
    }
    if !vue3_claim_tsconfig_path_materialization(path_bytes, type_resolver) {
        return None;
    }
    let relative = path_components
        .map(|component| component.as_os_str())
        .collect::<PathBuf>();
    debug_assert!(relative.as_os_str().as_encoded_bytes().len() <= path_bytes);
    type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&relative))
        .then_some(relative)
}

#[cfg(test)]
pub(crate) fn vue3_possible_project_input_paths(output_path: &Path) -> Vec<PathBuf> {
    let Some((stem, input_extensions)) = vue3_possible_project_input_path_parts(output_path) else {
        return Vec::new();
    };
    input_extensions
        .iter()
        .map(|extension| vue3_project_package_input_candidate(output_path, stem, extension))
        .collect()
}

fn vue3_possible_project_input_path_parts(output_path: &Path) -> Option<(&str, &[&str])> {
    let file_name = output_path.file_name().and_then(|name| name.to_str())?;
    let (output_extension, input_extensions): (&str, &[&str]) =
        if vue3_file_name_has_ascii_suffix(file_name, ".d.mts") {
            (".d.mts", &[".mts", ".mjs"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".mjs") {
            (".mjs", &[".mts", ".mjs"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".d.cts") {
            (".d.cts", &[".cts", ".cjs"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".cjs") {
            (".cjs", &[".cts", ".cjs"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".d.json.ts") {
            (".d.json.ts", &[".json"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".js") {
            (".js", &[".tsx", ".ts", ".jsx", ".js"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".json") {
            (".json", &[".json"])
        } else if vue3_file_name_has_ascii_suffix(file_name, ".d.ts") {
            (".d.ts", &[".tsx", ".ts", ".jsx", ".js"])
        } else {
            return None;
        };
    let stem = &file_name[..file_name.len() - output_extension.len()];
    Some((stem, input_extensions))
}

fn vue3_file_name_has_ascii_suffix(file_name: &str, suffix: &str) -> bool {
    file_name
        .get(file_name.len().saturating_sub(suffix.len())..)
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(suffix))
}

fn vue3_materialized_project_package_input_candidate(
    output_path: &Path,
    stem: &str,
    extension: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let parent = output_path.parent().unwrap_or_else(|| Path::new(""));
    let file_name_bytes = stem.len().saturating_add(extension.len());
    let path_bytes = parent
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .saturating_add(usize::from(
            !parent.as_os_str().is_empty() && file_name_bytes != 0,
        ))
        .saturating_add(file_name_bytes);
    let allocation_bytes = path_bytes.max(output_path.as_os_str().as_encoded_bytes().len());
    if !vue3_claim_tsconfig_path_materialization(allocation_bytes, type_resolver) {
        return None;
    }
    let candidate = vue3_project_package_input_candidate(output_path, stem, extension);
    debug_assert!(candidate.as_os_str().as_encoded_bytes().len() <= allocation_bytes);
    Some(candidate)
}

fn vue3_project_package_input_candidate(
    output_path: &Path,
    stem: &str,
    extension: &str,
) -> PathBuf {
    debug_assert!(extension.starts_with('.'));
    let mut file_name = String::with_capacity(stem.len().saturating_add(extension.len()));
    file_name.push_str(stem);
    file_name.push_str(extension);
    let mut candidate = output_path.to_path_buf();
    candidate.set_file_name(file_name);
    candidate
}

#[cfg(test)]
mod project_package_input_target_tests {
    use super::*;

    fn resolver_with_limits(limits: Vue3ExternalTypeLoadLimits) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    fn materialized_path_weight(paths: &[PathBuf]) -> usize {
        paths.iter().fold(0usize, |weight, path| {
            weight.saturating_add(
                std::mem::size_of::<PathBuf>()
                    .saturating_add(path.as_os_str().as_encoded_bytes().len()),
            )
        })
    }

    fn resolve_fixture(root: &Path, resolver: &Vue3TypeResolverContext) -> Option<PathBuf> {
        let package_dir = root.join("package");
        let source_dir = package_dir.join("src");
        let output_dir = package_dir.join("dist");
        resolve_vue3_project_package_input_target_with_mode(
            &source_dir.join("Comp.vue"),
            &package_dir,
            "./dist/leaf.js",
            &package_dir.join("tsconfig.json"),
            &Vue3TsconfigEmitPathOptions {
                root_dir: Some(source_dir),
                out_dir: Some(output_dir),
                declaration_dir: None,
                composite: None,
            },
            Vue3TypeResolutionMode::Import,
            resolver,
        )
    }

    #[test]
    fn project_package_input_mapping_honors_exact_resource_boundaries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("package").join("src");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        let leaf = source_dir.join("leaf.ts");
        std::fs::write(&leaf, "export interface Leaf {}").expect("write source target");
        let package_dir = dir.path().join("package");
        let materialized_paths = [
            package_dir.join("dist").join("leaf.js"),
            source_dir.clone(),
            PathBuf::from("leaf.js"),
            source_dir.join("leaf.js"),
            source_dir.join("leaf.tsx"),
            source_dir.join("leaf.ts"),
        ];
        let materialization_entries = materialized_paths.len();
        let materialization_weight = materialized_path_weight(&materialized_paths);

        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 1,
            max_metadata_resolution_path_probes: 3,
            max_tsconfig_materialization_entries: materialization_entries,
            max_tsconfig_materialization_weight: materialization_weight,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(resolve_fixture(dir.path(), &exact), Some(leaf.clone()));
        let stats = exact.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, 1);
        assert_eq!(stats.metadata_resolution_path_probes, 3);
        assert_eq!(
            stats.tsconfig_materialization_entries,
            materialization_entries
        );
        assert_eq!(stats.tsconfig_materialization_weight, materialization_weight);
        assert!(!exact.external_type_session.metadata_is_blocked());

        let no_fanout = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &no_fanout).is_none());
        assert!(no_fanout.external_type_session.metadata_is_blocked());

        let two_probes = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_resolution_path_probes: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &two_probes).is_none());
        assert!(two_probes.external_type_session.metadata_is_blocked());

        let no_materialization = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_materialization_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &no_materialization).is_none());
        assert!(no_materialization
            .external_type_session
            .metadata_is_blocked());

        let short_materialization_entries = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_materialization_entries: materialization_entries - 1,
            max_tsconfig_materialization_weight: materialization_weight,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &short_materialization_entries).is_none());
        assert!(short_materialization_entries
            .external_type_session
            .metadata_is_blocked());

        let short_materialization_weight = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_materialization_entries: materialization_entries,
            max_tsconfig_materialization_weight: materialization_weight - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &short_materialization_weight).is_none());
        assert!(short_materialization_weight
            .external_type_session
            .metadata_is_blocked());

        let longest_path = [
            package_dir.join("dist").join("leaf.js"),
            package_dir.join("src"),
            package_dir.join("src").join("leaf.tsx"),
            package_dir.join("src").join("leaf.ts"),
        ]
        .iter()
        .map(|path| normalize_path_string(path).len())
        .max()
        .expect("fixture paths");
        let exact_path = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: longest_path,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(resolve_fixture(dir.path(), &exact_path), Some(leaf));
        assert!(!exact_path.external_type_session.metadata_is_blocked());

        let short_path = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: longest_path - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &short_path).is_none());
        assert!(short_path.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn project_package_input_mapping_bounds_joined_paths_before_allocation() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("package");
        let source_root = package_dir.join("a-deliberately-long-source-root");
        let output_dir = package_dir.join("d");
        let importer = source_root.join("Comp.vue");
        let config_path = package_dir.join("tsconfig.json");
        let options = Vue3TsconfigEmitPathOptions {
            root_dir: Some(source_root.clone()),
            out_dir: Some(output_dir.clone()),
            declaration_dir: None,
            composite: None,
        };
        let final_path = output_dir.join("entry.json");
        let fragment = PathBuf::from("entry.json");
        let possible_input = source_root.join("entry.json");
        let exact_paths = [
            final_path.clone(),
            source_root.clone(),
            fragment,
            possible_input.clone(),
        ];
        let exact_weight = materialized_path_weight(&exact_paths);
        let possible_input_bytes = possible_input.as_os_str().as_encoded_bytes().len();
        let final_path_bytes = final_path.as_os_str().as_encoded_bytes().len();
        assert!(possible_input_bytes > final_path_bytes);

        let resolve = |resolver: &Vue3TypeResolverContext| {
            resolve_vue3_project_package_input_target_with_mode(
                &importer,
                &package_dir,
                "./d/entry.json",
                &config_path,
                &options,
                Vue3TypeResolutionMode::Import,
                resolver,
            )
        };
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: possible_input_bytes,
            max_tsconfig_materialization_entries: exact_paths.len(),
            max_tsconfig_materialization_weight: exact_weight,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve(&exact).is_none());
        let stats = exact.external_type_session.stats();
        assert_eq!(stats.tsconfig_materialization_entries, exact_paths.len());
        assert_eq!(stats.tsconfig_materialization_weight, exact_weight);
        assert_eq!(stats.metadata_resolution_path_probes, 0);
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short_join = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: possible_input_bytes - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve(&short_join).is_none());
        let stats = short_join.external_type_session.stats();
        assert_eq!(stats.tsconfig_materialization_entries, exact_paths.len() - 1);
        assert_eq!(stats.metadata_resolution_path_probes, 0);
        assert!(short_join.external_type_session.metadata_is_blocked());

        let short_final = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: final_path_bytes - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve(&short_final).is_none());
        let stats = short_final.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, 0);
        assert_eq!(stats.tsconfig_materialization_entries, 0);
        assert_eq!(stats.metadata_resolution_path_probes, 0);
        assert!(short_final.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn project_package_input_phases_materialize_only_relevant_candidates() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("package");
        let source_root = package_dir.join("src");
        let output_dir = package_dir.join("dist");
        let importer = source_root.join("Comp.vue");
        let config_path = package_dir.join("tsconfig.json");
        let options = Vue3TsconfigEmitPathOptions {
            root_dir: Some(source_root.clone()),
            out_dir: Some(output_dir.clone()),
            declaration_dir: None,
            composite: None,
        };
        for phase in [
            Vue3PackageResolutionPhase::Types,
            Vue3PackageResolutionPhase::JavaScript,
        ] {
            let candidate_paths = match phase {
                Vue3PackageResolutionPhase::Types => {
                    [source_root.join("leaf.tsx"), source_root.join("leaf.ts")]
                }
                Vue3PackageResolutionPhase::JavaScript => {
                    [source_root.join("leaf.jsx"), source_root.join("leaf.js")]
                }
            };
            let materialized_paths = [
                output_dir.join("leaf.js"),
                source_root.clone(),
                PathBuf::from("leaf.js"),
                source_root.join("leaf.js"),
                candidate_paths[0].clone(),
                candidate_paths[1].clone(),
            ];
            let materialization_weight = materialized_path_weight(&materialized_paths);
            let resolver = resolver_with_limits(Vue3ExternalTypeLoadLimits {
                max_metadata_resolution_path_probes: 2,
                max_tsconfig_materialization_entries: materialized_paths.len(),
                max_tsconfig_materialization_weight: materialization_weight,
                ..Vue3ExternalTypeLoadLimits::default()
            });
            assert!(resolve_vue3_project_package_input_target_for_phase_with_mode(
                &importer,
                &package_dir,
                "./dist/leaf.js",
                &config_path,
                &options,
                Vue3TypeResolutionMode::Import,
                phase,
                &resolver,
            )
            .is_none());
            let stats = resolver.external_type_session.stats();
            assert_eq!(
                stats.tsconfig_materialization_entries,
                materialized_paths.len()
            );
            assert_eq!(
                stats.tsconfig_materialization_weight,
                materialization_weight
            );
            assert_eq!(stats.metadata_resolution_path_probes, 2);
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }

    #[test]
    fn project_package_source_root_guesses_bound_ancestor_materialization() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("workspace").join("package");
        let importer = package_dir.join("src").join("Comp.vue");
        let config_path = package_dir.join("tsconfig.json");
        let ancestor_roots = package_dir.ancestors().collect::<Vec<_>>();
        let root_count = ancestor_roots.len();
        let max_root_bytes = ancestor_roots
            .iter()
            .map(|root| root.as_os_str().as_encoded_bytes().len())
            .max()
            .expect("package path has ancestors");
        let materialization_weight = ancestor_roots.iter().fold(0usize, |weight, root| {
            weight.saturating_add(
                std::mem::size_of::<PathBuf>()
                    .saturating_add(root.as_os_str().as_encoded_bytes().len()),
            )
        });
        let mut expected = ancestor_roots
            .iter()
            .map(|root| normalize_path_components((*root).to_path_buf()))
            .collect::<Vec<_>>();
        expected.reverse();
        let resolver = |limits| Vue3TypeResolverContext {
            typescript_version: (5, 9, 0).into(),
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        };

        let exact = resolver(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: root_count,
            max_tsconfig_materialization_entries: root_count,
            max_tsconfig_materialization_weight: materialization_weight,
            max_generated_path_bytes: max_root_bytes,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_project_package_source_root_guesses(
                &importer,
                &package_dir,
                &config_path,
                &Vue3TsconfigEmitPathOptions::default(),
                &exact,
            ),
            Some(expected),
        );
        let stats = exact.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, root_count);
        assert_eq!(stats.tsconfig_materialization_entries, root_count);
        assert_eq!(
            stats.tsconfig_materialization_weight,
            materialization_weight
        );
        assert!(!exact.external_type_session.metadata_is_blocked());

        let short_entries = resolver(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: root_count,
            max_tsconfig_materialization_entries: root_count - 1,
            max_tsconfig_materialization_weight: materialization_weight,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_project_package_source_root_guesses(
            &importer,
            &package_dir,
            &config_path,
            &Vue3TsconfigEmitPathOptions::default(),
            &short_entries,
        )
        .is_none());
        assert!(short_entries
            .external_type_session
            .metadata_is_blocked());
        let stats = short_entries.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, root_count);
        assert_eq!(stats.tsconfig_materialization_entries, root_count - 1);

        let short_fanout = resolver(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: root_count - 1,
            max_tsconfig_materialization_entries: root_count,
            max_tsconfig_materialization_weight: materialization_weight,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_project_package_source_root_guesses(
            &importer,
            &package_dir,
            &config_path,
            &Vue3TsconfigEmitPathOptions::default(),
            &short_fanout,
        )
        .is_none());
        assert!(short_fanout.external_type_session.metadata_is_blocked());
        let stats = short_fanout.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, root_count - 1);
        assert_eq!(stats.tsconfig_materialization_entries, root_count - 1);

        let short_weight = resolver(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: root_count,
            max_tsconfig_materialization_entries: root_count,
            max_tsconfig_materialization_weight: materialization_weight - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_project_package_source_root_guesses(
            &importer,
            &package_dir,
            &config_path,
            &Vue3TsconfigEmitPathOptions::default(),
            &short_weight,
        )
        .is_none());
        assert!(short_weight.external_type_session.metadata_is_blocked());
        let stats = short_weight.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, root_count);
        assert_eq!(stats.tsconfig_materialization_entries, root_count - 1);
        assert_eq!(
            stats.tsconfig_materialization_weight,
            materialization_weight - 1
        );

        let short_path = resolver(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: root_count,
            max_tsconfig_materialization_entries: root_count,
            max_tsconfig_materialization_weight: materialization_weight,
            max_generated_path_bytes: max_root_bytes - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_project_package_source_root_guesses(
            &importer,
            &package_dir,
            &config_path,
            &Vue3TsconfigEmitPathOptions::default(),
            &short_path,
        )
        .is_none());
        assert!(short_path.external_type_session.metadata_is_blocked());
        let stats = short_path.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, 1);
        assert_eq!(stats.tsconfig_materialization_entries, 0);
    }

    #[test]
    fn project_package_input_mapping_preserves_source_root_order_within_passes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("workspace").join("package");
        let source_dir = package_dir.join("src");
        let output_dir = package_dir.join("dist");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        let stem = format!(
            "vuec-root-order-{}",
            dir.path()
                .file_name()
                .and_then(|name| name.to_str())
                .expect("temp directory name")
        );
        let early_javascript = dir.path().join(format!("{stem}.js"));
        let late_typescript = package_dir.join(format!("{stem}.ts"));
        std::fs::write(&early_javascript, "export const early = true;")
            .expect("write earlier JavaScript input");
        std::fs::write(&late_typescript, "export const late = true;")
            .expect("write later TypeScript input");
        let importer = source_dir.join("Comp.vue");
        let config_path = package_dir.join("tsconfig.json");
        let target = format!("./dist/{stem}.js");
        let options = Vue3TsconfigEmitPathOptions {
            root_dir: None,
            out_dir: Some(output_dir),
            declaration_dir: None,
            composite: None,
        };

        assert_eq!(
            resolve_vue3_project_package_input_target_with_mode(
                &importer,
                &package_dir,
                &target,
                &config_path,
                &options,
                Vue3TypeResolutionMode::Import,
                &Vue3TypeResolverContext::default(),
            ),
            Some(early_javascript.clone()),
        );
        assert_eq!(
            resolve_vue3_project_package_input_target_for_phase_with_mode(
                &importer,
                &package_dir,
                &target,
                &config_path,
                &options,
                Vue3TypeResolutionMode::Import,
                Vue3PackageResolutionPhase::Types,
                &Vue3TypeResolverContext::default(),
            ),
            Some(late_typescript),
        );
        assert_eq!(
            resolve_vue3_project_package_input_target_for_phase_with_mode(
                &importer,
                &package_dir,
                &target,
                &config_path,
                &options,
                Vue3TypeResolutionMode::Import,
                Vue3PackageResolutionPhase::JavaScript,
                &Vue3TypeResolverContext::default(),
            ),
            Some(early_javascript),
        );
    }

    #[test]
    fn project_package_input_mapping_requires_exact_original_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("package");
        let source_dir = package_dir.join("src");
        let output_dir = package_dir.join("dist");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            source_dir.join("declaration.d.ts"),
            "export interface DeclarationOnly {}",
        )
        .expect("write adjacent declaration");
        std::fs::write(
            source_dir.join("suffix.native.ts"),
            "export interface SuffixOnly {}",
        )
        .expect("write module-suffixed input");
        let importer = source_dir.join("Comp.vue");
        let config_path = package_dir.join("tsconfig.json");
        let options = Vue3TsconfigEmitPathOptions {
            root_dir: Some(source_dir),
            out_dir: Some(output_dir),
            declaration_dir: None,
            composite: None,
        };
        let resolver = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([
                ".native".to_string(),
                String::new(),
            ]),
            ..Vue3TypeResolverContext::default()
        };

        for target in ["./dist/declaration.js", "./dist/suffix.js"] {
            assert!(resolve_vue3_project_package_input_target_with_mode(
                &importer,
                &package_dir,
                target,
                &config_path,
                &options,
                Vue3TypeResolutionMode::Import,
                &resolver,
            )
            .is_none());
        }
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            8,
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn project_package_input_loader_miss_falls_back_to_emitted_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("package");
        let source_dir = package_dir.join("src");
        let output_dir = package_dir.join("dist");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::create_dir_all(&output_dir).expect("create output directory");
        std::fs::write(source_dir.join("leaf.ts"), "export interface Input {}")
            .expect("write exact input candidate");
        let emitted = output_dir.join("leaf.native.d.ts");
        std::fs::write(&emitted, "export interface Emitted {}")
            .expect("write module-suffixed declaration output");
        let importer = source_dir.join("Comp.vue");
        let options = (
            package_dir.join("tsconfig.json"),
            Vue3TsconfigEmitPathOptions {
                root_dir: Some(source_dir),
                out_dir: Some(output_dir),
                declaration_dir: None,
                composite: None,
            },
        );
        let resolver = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([".native".to_string()]),
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_package_relative_target_with_project_input(
                &importer,
                &package_dir,
                "./dist/leaf.js",
                Some(&options),
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            Some(emitted),
        );
    }

    #[test]
    fn project_package_import_mapping_is_cached_without_repeating_metadata_work() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir.path().join("package");
        let source_dir = package_dir.join("src");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            package_dir.join("package.json"),
            r##"{"imports":{"#leaf":"./dist/leaf.js"}}"##,
        )
        .expect("write package manifest");
        std::fs::write(
            package_dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler","rootDir":"./src","outDir":"./dist"}}"#,
        )
        .expect("write project config");
        let leaf = source_dir.join("leaf.ts");
        std::fs::write(&leaf, "export interface Leaf {}").expect("write source target");
        let importer = source_dir.join("Comp.vue");
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_type_import(&importer.to_string_lossy(), "#leaf", &resolver),
            Some(leaf.clone())
        );
        let first = resolver.external_type_session.stats();
        assert_eq!(
            resolve_vue3_type_import(&importer.to_string_lossy(), "#leaf", &resolver),
            Some(leaf)
        );
        let cached = resolver.external_type_session.stats();
        assert_eq!(cached.resolution_cache_hits, first.resolution_cache_hits + 1);
        assert_eq!(cached.metadata_files_read, first.metadata_files_read);
        assert_eq!(
            cached.metadata_fanout_entries,
            first.metadata_fanout_entries
        );
        assert_eq!(
            cached.metadata_resolution_path_probes,
            first.metadata_resolution_path_probes
        );
        assert_eq!(
            cached.tsconfig_materialization_entries,
            first.tsconfig_materialization_entries
        );
        assert_eq!(
            cached.tsconfig_materialization_weight,
            first.tsconfig_materialization_weight
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }
}
