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
    let is_relative = vue3_type_import_source_is_relative(source);
    match type_resolver
        .external_type_session
        .begin_type_import_resolution(
            Vue3TypeResolutionKind::Module(resolution_mode),
            filename,
            source,
            &type_resolver.typescript_version,
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
                type_resolver,
                is_relative,
            );
            type_resolver
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
    match resolve_vue3_dependency_package_self_reference_with_mode(
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
    resolve_vue3_bare_type_import_with_mode(filename, source, resolution_mode, type_resolver)
}

enum Vue3PackageSelfReferenceResolution {
    NotApplicable,
    Resolved(PathBuf),
    Rejected,
    MetadataBlocked,
}

fn resolve_vue3_dependency_package_self_reference_with_mode(
    filename: &str,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageSelfReferenceResolution {
    let filename = Path::new(filename);
    // Local project self-references also need tsconfig output-to-input remapping.
    if !vue3_path_contains_node_modules(filename) {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    }
    let Some((package_name, subpath)) = vue3_package_import_parts(source) else {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    };
    let (package_dir, manifest) = match vue3_package_scope_for_path(
        filename,
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
    if manifest.name.as_deref() != Some(package_name.as_str())
        || manifest.exports.as_ref().is_none_or(serde_json::Value::is_null)
    {
        return Vue3PackageSelfReferenceResolution::NotApplicable;
    }

    match resolve_vue3_package_json_type_entry_with_mode(
        &package_dir,
        subpath.as_deref(),
        resolution_mode,
        type_resolver,
    ) {
        Vue3PackageJsonTypeResolution::Resolved(path) => {
            Vue3PackageSelfReferenceResolution::Resolved(path)
        }
        Vue3PackageJsonTypeResolution::Blocked
        | Vue3PackageJsonTypeResolution::NoPackageJson
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {
            if type_resolver.external_type_session.metadata_is_blocked() {
                Vue3PackageSelfReferenceResolution::MetadataBlocked
            } else {
                Vue3PackageSelfReferenceResolution::Rejected
            }
        }
    }
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
    let candidate = normalize_path_components(base.join(source));
    resolve_vue3_type_import_path_with_mode(&candidate, resolution_mode, type_resolver)
}
