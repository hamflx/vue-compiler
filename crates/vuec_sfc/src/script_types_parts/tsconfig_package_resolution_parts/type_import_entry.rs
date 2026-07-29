#[cfg(test)]
pub(crate) fn resolve_vue3_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_with_mode(
        filename,
        source,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_type_import_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_with_request(
        filename,
        source,
        resolution_mode,
        false,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_type_import_with_explicit_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_with_request(
        filename,
        source,
        resolution_mode,
        true,
        type_resolver,
    )
}

fn resolve_vue3_type_import_with_request(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    explicit_mode: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    // Bundler did not receive explicit resolution modes until TypeScript 5.3.
    let resolution_mode = if explicit_mode
        && type_resolver.typescript_version < (5, 3, 0).into()
        && type_resolver.module_resolution == Vue3TypeModuleResolutionKind::Bundler
    {
        Vue3TypeResolutionMode::Import
    } else {
        resolution_mode
    };
    let mut request_resolver = type_resolver.clone();
    request_resolver.active_package_json_features = Some(
        type_resolver.package_json_features_for_request(explicit_mode),
    );
    let is_relative = vue3_type_import_source_is_relative(source);
    match request_resolver
        .external_type_session
        .begin_type_import_resolution(
            Vue3TypeResolutionKind::Module {
                mode: resolution_mode,
                explicit_mode,
            },
            filename,
            source,
            &request_resolver,
            is_relative,
        )
    {
        Vue3TypeImportResolutionLoad::Ready(resolution) => resolution,
        Vue3TypeImportResolutionLoad::Failed => None,
        Vue3TypeImportResolutionLoad::Start {
            cache_key,
            failure_epoch,
        } => {
            let resolution = resolve_vue3_type_import_uncached(
                filename,
                source,
                resolution_mode,
                &request_resolver,
                is_relative,
            );
            request_resolver
                .external_type_session
                .finish_type_import_resolution(
                    cache_key,
                    resolution,
                    failure_epoch,
                    is_relative,
                )
        }
    }
}

fn resolve_vue3_type_import_uncached(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    is_relative: bool,
) -> Option<PathBuf> {
    if is_relative {
        return resolve_vue3_relative_type_import_with_mode(
            filename,
            source,
            resolution_mode,
            type_resolver,
        );
    }
    if let Some(resolved) = resolve_vue3_tsconfig_type_import_with_mode(
        filename,
        source,
        resolution_mode,
        type_resolver,
    ) {
        return Some(resolved);
    }
    if type_resolver.module_resolution == Vue3TypeModuleResolutionKind::Classic {
        return resolve_vue3_classic_type_import_with_mode(
            filename,
            source,
            resolution_mode,
            type_resolver,
        );
    }
    let package_json_features = type_resolver.package_json_features();
    if package_json_features.imports {
        match resolve_vue3_package_imports_with_mode(
            filename,
            source,
            resolution_mode,
            type_resolver,
        ) {
            Vue3PackageImportsResolution::Resolved(path) => return Some(path),
            Vue3PackageImportsResolution::Rejected | Vue3PackageImportsResolution::Blocked => {
                return None;
            }
            Vue3PackageImportsResolution::NotApplicable => {}
        }
    }
    if package_json_features.self_name {
        match resolve_vue3_package_self_reference_with_mode(
            filename,
            source,
            resolution_mode,
            type_resolver,
        ) {
            Vue3PackageSelfReferenceResolution::Resolved(path) => return Some(path),
            Vue3PackageSelfReferenceResolution::Rejected
            | Vue3PackageSelfReferenceResolution::MetadataBlocked => return None,
            Vue3PackageSelfReferenceResolution::NotApplicable => {}
        }
    }
    resolve_vue3_bare_type_import_with_mode(filename, source, resolution_mode, type_resolver)
}

enum Vue3PackageImportsResolution {
    NotApplicable,
    Resolved(PathBuf),
    Rejected,
    Blocked,
}

fn resolve_vue3_package_imports_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageImportsResolution {
    if !source.starts_with('#') {
        return Vue3PackageImportsResolution::NotApplicable;
    }
    let filename = normalize_path_components(PathBuf::from(filename));
    if type_resolver.typescript_version < (4, 7, 0).into() {
        return Vue3PackageImportsResolution::Rejected;
    }
    if !vue3_package_import_specifier_is_safe_for_resolver(source, type_resolver) {
        return Vue3PackageImportsResolution::Rejected;
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(source)
    {
        return Vue3PackageImportsResolution::Blocked;
    }
    let (package_dir, manifest) = match vue3_package_scope_for_path(
        &filename,
        &type_resolver.external_type_session,
    ) {
        Vue3PackageScopeResolution::Found {
            package_dir,
            manifest,
        } => (package_dir, manifest),
        Vue3PackageScopeResolution::Missing => return Vue3PackageImportsResolution::Rejected,
        Vue3PackageScopeResolution::MetadataBlocked => {
            return Vue3PackageImportsResolution::Blocked;
        }
    };
    let Some(imports) = manifest
        .imports
        .as_ref()
        .filter(|imports| vue3_package_json_value_is_truthy(imports))
    else {
        return Vue3PackageImportsResolution::Rejected;
    };
    let is_project_package = !vue3_path_contains_node_modules(&package_dir);
    let emit_path_options = if is_project_package {
        vue3_tsconfig_emit_path_options(&filename.to_string_lossy(), &package_dir, type_resolver)
    } else {
        None
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageImportsResolution::Blocked;
    }
    let _resolution_guard = match type_resolver
        .external_type_session
        .begin_package_import_resolution(
            &package_dir,
            source,
            resolution_mode,
            type_resolver,
        )
    {
        Vue3PackageImportResolutionLoad::Ready(guard) => guard,
        Vue3PackageImportResolutionLoad::Cycle => {
            return Vue3PackageImportsResolution::Rejected;
        }
        Vue3PackageImportResolutionLoad::Blocked => {
            return Vue3PackageImportsResolution::Blocked;
        }
    };
    let package_filename = package_dir.join("package.json");
    let mut resolved = None;
    let result = visit_vue3_package_imports_type_targets(
        imports,
        source,
        resolution_mode,
        type_resolver,
        &mut |target| {
            let failure_epoch = type_resolver.external_type_session.failure_epoch();
            let candidate = if target.starts_with("./") {
                resolve_vue3_package_relative_target_with_project_input(
                    &filename,
                    &package_dir,
                    target,
                    emit_path_options.as_ref(),
                    resolution_mode,
                    type_resolver,
                )
            } else {
                resolve_vue3_type_import_with_mode(
                    &package_filename.to_string_lossy(),
                    target,
                    resolution_mode,
                    type_resolver,
                )
            };
            if type_resolver.external_type_session.metadata_is_blocked()
                || type_resolver.external_type_session.failure_epoch() != failure_epoch
            {
                Vue3PackageTargetVisit::Blocked
            } else if let Some(candidate) = candidate {
                resolved = Some(candidate);
                Vue3PackageTargetVisit::Resolved
            } else {
                Vue3PackageTargetVisit::Missing
            }
        },
    );
    match (result, resolved) {
        (Vue3PackageTargetVisit::Resolved, Some(path)) => {
            Vue3PackageImportsResolution::Resolved(path)
        }
        (Vue3PackageTargetVisit::Blocked, _) => Vue3PackageImportsResolution::Blocked,
        _ if type_resolver.external_type_session.metadata_is_blocked() => {
            Vue3PackageImportsResolution::Blocked
        }
        _ => Vue3PackageImportsResolution::Rejected,
    }
}

enum Vue3PackageSelfReferenceResolution {
    NotApplicable,
    Resolved(PathBuf),
    Rejected,
    MetadataBlocked,
}

fn resolve_vue3_package_self_reference_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageSelfReferenceResolution {
    let filename = normalize_path_components(PathBuf::from(filename));
    let Some((package_name, subpath)) = vue3_package_import_parts(source) else {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    };
    if type_resolver.typescript_version < (4, 7, 0).into() {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    }
    let (package_dir, manifest) = match vue3_package_scope_for_path(
        &filename,
        &type_resolver.external_type_session,
    ) {
        Vue3PackageScopeResolution::Found {
            package_dir,
            manifest,
        } => (package_dir, manifest),
        Vue3PackageScopeResolution::Missing => {
            return Vue3PackageSelfReferenceResolution::NotApplicable;
        }
        Vue3PackageScopeResolution::MetadataBlocked => {
            return Vue3PackageSelfReferenceResolution::MetadataBlocked;
        }
    };
    if manifest.name.as_deref() != Some(package_name.as_str()) {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    }
    let Some(exports) = manifest
        .exports
        .as_ref()
        .filter(|exports| vue3_package_json_value_is_truthy(exports))
    else {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    };
    let emit_path_options = if vue3_path_contains_node_modules(&package_dir) {
        None
    } else {
        vue3_tsconfig_emit_path_options(&filename.to_string_lossy(), &package_dir, type_resolver)
    };
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageSelfReferenceResolution::MetadataBlocked;
    }
    let Some(_resolution_guard) = type_resolver
        .external_type_session
        .begin_package_resolution(&package_dir)
    else {
        return Vue3PackageSelfReferenceResolution::MetadataBlocked;
    };
    // TypeScript treats a root `.` null as falsy before entering target traversal.
    let root_null_target = subpath.is_none()
        && exports
            .as_object()
            .and_then(|object| object.get("."))
            .is_some_and(serde_json::Value::is_null);

    // Since TypeScript 5.2, project JS inputs beat their emitted declarations per target.
    let single_pass = type_resolver.typescript_version >= (5, 2, 0).into()
        && type_resolver.allow_js
        && !vue3_path_contains_node_modules(
            filename.parent().unwrap_or_else(|| Path::new("")),
        );
    let passes: &[Option<Vue3PackageResolutionPhase>] = if single_pass {
        &[None]
    } else {
        &[
            Some(Vue3PackageResolutionPhase::Types),
            Some(Vue3PackageResolutionPhase::JavaScript),
        ]
    };
    for phase in passes.iter().copied() {
        let mut resolved = None;
        let result = visit_vue3_package_exports_type_targets(
            exports,
            subpath.as_deref(),
            resolution_mode,
            type_resolver,
            &mut |target| {
                let failure_epoch = type_resolver.external_type_session.failure_epoch();
                let candidate = if let Some(phase) = phase {
                    resolve_vue3_package_relative_target_with_project_input_for_phase(
                        &filename,
                        &package_dir,
                        target,
                        emit_path_options.as_ref(),
                        resolution_mode,
                        phase,
                        type_resolver,
                    )
                } else {
                    resolve_vue3_package_relative_target_with_project_input(
                        &filename,
                        &package_dir,
                        target,
                        emit_path_options.as_ref(),
                        resolution_mode,
                        type_resolver,
                    )
                };
                if type_resolver.external_type_session.metadata_is_blocked()
                    || type_resolver.external_type_session.failure_epoch() != failure_epoch
                {
                    Vue3PackageTargetVisit::Blocked
                } else if let Some(candidate) = candidate {
                    resolved = Some(candidate);
                    Vue3PackageTargetVisit::Resolved
                } else {
                    Vue3PackageTargetVisit::Missing
                }
            },
        );
        match (result, resolved) {
            (Vue3PackageTargetVisit::Resolved, Some(path)) => {
                return Vue3PackageSelfReferenceResolution::Resolved(path);
            }
            (Vue3PackageTargetVisit::Blocked, _) => {
                return Vue3PackageSelfReferenceResolution::MetadataBlocked;
            }
            _ if type_resolver.external_type_session.metadata_is_blocked() => {
                return Vue3PackageSelfReferenceResolution::MetadataBlocked;
            }
            (Vue3PackageTargetVisit::NullTarget, _)
                if vue3_package_null_target_stops_fallback(type_resolver) && !root_null_target =>
            {
                return Vue3PackageSelfReferenceResolution::Rejected;
            }
            (Vue3PackageTargetVisit::Missing, _) => {}
            _ => return Vue3PackageSelfReferenceResolution::NotApplicable,
        }
    }
    Vue3PackageSelfReferenceResolution::NotApplicable
}

fn resolve_vue3_relative_type_import_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let base = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let candidate = normalize_path_components(base.join(source.replace('\\', "/")));
    let Some((matched_root, suffix)) = type_resolver
        .root_dirs
        .iter()
        .filter_map(|root_dir| {
            candidate.strip_prefix(root_dir).ok().and_then(|suffix| {
                (!suffix.as_os_str().is_empty()).then_some((root_dir, suffix))
            })
        })
        .max_by_key(|(root_dir, _)| root_dir.as_os_str().as_encoded_bytes().len())
    else {
        return resolve_vue3_type_import_path_with_mode(
            &candidate,
            resolution_mode,
            type_resolver,
        );
    };
    let failure_epoch = type_resolver.external_type_session.failure_epoch();
    if !type_resolver
        .external_type_session
        .claim_metadata_fanout_entry()
    {
        return None;
    }
    let resolved =
        resolve_vue3_type_import_path_with_mode(&candidate, resolution_mode, type_resolver);
    if resolved.is_some()
        || type_resolver.external_type_session.failure_epoch() != failure_epoch
    {
        return resolved;
    }
    for root_dir in type_resolver.root_dirs.iter() {
        if root_dir == matched_root {
            continue;
        }
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        let candidate = normalize_path_components(root_dir.join(suffix));
        let failure_epoch = type_resolver.external_type_session.failure_epoch();
        let resolved =
            resolve_vue3_type_import_path_with_mode(&candidate, resolution_mode, type_resolver);
        if resolved.is_some()
            || type_resolver.external_type_session.failure_epoch() != failure_epoch
        {
            return resolved;
        }
    }
    None
}
