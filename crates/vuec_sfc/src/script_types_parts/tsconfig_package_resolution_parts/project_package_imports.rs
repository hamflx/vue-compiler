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
    let target =
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, type_resolver)?;
    let path = Path::new(&target);
    let path = if path.is_absolute() {
        normalize_path_components(PathBuf::from(&target))
    } else {
        normalize_path_components(config_dir.join(&target))
    };
    type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&normalize_path_string(&path))
        .then_some(path)
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
    if options.out_dir.is_none() && options.declaration_dir.is_none() {
        return None;
    }
    let relative_target = target.strip_prefix("./")?;
    let final_path = normalize_path_components(package_dir.join(relative_target));
    let final_path_text = normalize_path_string(&final_path);
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(&final_path_text)
    {
        return None;
    }
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
            let Some(path_fragment) = vue3_path_relative_to(&final_path, output_dir) else {
                continue;
            };
            if path_fragment.as_os_str().is_empty() {
                continue;
            }
            let possible_input = normalize_path_components(source_root.join(path_fragment));
            for candidate in vue3_possible_project_input_paths(&possible_input) {
                if !type_resolver
                    .external_type_session
                    .metadata_path_is_within_limit(&normalize_path_string(&candidate))
                {
                    return None;
                }
                let resolved = resolve_vue3_metadata_type_import_path_with_mode(
                    &candidate,
                    resolution_mode,
                    type_resolver,
                );
                if resolved.is_some() || type_resolver.external_type_session.metadata_is_blocked()
                {
                    return resolved;
                }
            }
        }
    }
    None
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
        input
    } else {
        vue3_package_export_type_path_with_mode(
            package_dir,
            target,
            resolution_mode,
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
    let roots = if let Some(root_dir) = &options.root_dir {
        vec![root_dir.clone()]
    } else if options.composite == Some(true)
        || type_resolver.typescript_version >= (6, 0, 0).into()
    {
        vec![config_dir.to_path_buf()]
    } else {
        let importer_dir = importer.parent().unwrap_or_else(|| Path::new(""));
        let common = importer_dir
            .ancestors()
            .find(|ancestor| package_dir.starts_with(ancestor))?;
        let mut guesses = common
            .ancestors()
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        guesses.reverse();
        guesses
    };
    let mut bounded = Vec::with_capacity(roots.len());
    for root in roots {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        let root = normalize_path_components(root);
        if !type_resolver
            .external_type_session
            .metadata_path_is_within_limit(&normalize_path_string(&root))
        {
            return None;
        }
        bounded.push(root);
    }
    Some(bounded)
}

fn vue3_path_relative_to(path: &Path, base: &Path) -> Option<PathBuf> {
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
    Some(path_components.map(|component| component.as_os_str()).collect())
}

pub(crate) fn vue3_possible_project_input_paths(output_path: &Path) -> Vec<PathBuf> {
    let Some(file_name) = output_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let lower = file_name.to_ascii_lowercase();
    let (output_extension, input_extensions): (&str, &[&str]) =
        if lower.ends_with(".d.mts") {
            (".d.mts", &[".mts", ".mjs"])
        } else if lower.ends_with(".mjs") {
            (".mjs", &[".mts", ".mjs"])
        } else if lower.ends_with(".d.cts") {
            (".d.cts", &[".cts", ".cjs"])
        } else if lower.ends_with(".cjs") {
            (".cjs", &[".cts", ".cjs"])
        } else if lower.ends_with(".d.json.ts") {
            (".d.json.ts", &[".json"])
        } else if lower.ends_with(".js") {
            (".js", &[".tsx", ".ts", ".jsx", ".js"])
        } else if lower.ends_with(".json") {
            (".json", &[".json"])
        } else if lower.ends_with(".d.ts") {
            (".d.ts", &[".tsx", ".ts", ".jsx", ".js"])
        } else {
            return Vec::new();
        };
    let stem = &file_name[..file_name.len() - output_extension.len()];
    input_extensions
        .iter()
        .map(|extension| {
            let mut candidate = output_path.to_path_buf();
            candidate.set_file_name(format!("{stem}{extension}"));
            candidate
        })
        .collect()
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

        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 1,
            max_metadata_resolution_path_probes: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(resolve_fixture(dir.path(), &exact), Some(leaf.clone()));
        let stats = exact.external_type_session.stats();
        assert_eq!(stats.metadata_fanout_entries, 1);
        assert_eq!(stats.metadata_resolution_path_probes, 2);
        assert!(!exact.external_type_session.metadata_is_blocked());

        let no_fanout = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &no_fanout).is_none());
        assert!(no_fanout.external_type_session.metadata_is_blocked());

        let one_probe = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_resolution_path_probes: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_fixture(dir.path(), &one_probe).is_none());
        assert!(one_probe.external_type_session.metadata_is_blocked());

        let package_dir = dir.path().join("package");
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
            r#"{"compilerOptions":{"rootDir":"./src","outDir":"./dist"}}"#,
        )
        .expect("write project config");
        let leaf = source_dir.join("leaf.ts");
        std::fs::write(&leaf, "export interface Leaf {}").expect("write source target");
        let importer = source_dir.join("Comp.vue");
        let resolver = Vue3TypeResolverContext::default();

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
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }
}
