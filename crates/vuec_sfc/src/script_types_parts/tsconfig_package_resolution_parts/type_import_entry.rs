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
    resolve_vue3_bare_type_import_with_mode(filename, source, resolution_mode, type_resolver)
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
