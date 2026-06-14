pub(crate) fn resolve_vue3_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if vue3_type_import_source_is_relative(source) {
        return resolve_vue3_relative_type_import(filename, source, type_resolver);
    }
    if let Some(resolved) = resolve_vue3_tsconfig_type_import(filename, source, type_resolver) {
        return Some(resolved);
    }
    resolve_vue3_bare_type_import(filename, source, type_resolver)
}

pub(crate) fn resolve_vue3_relative_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let base = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let candidate = normalize_path_components(base.join(source));
    resolve_vue3_type_import_path(&candidate, type_resolver)
}
