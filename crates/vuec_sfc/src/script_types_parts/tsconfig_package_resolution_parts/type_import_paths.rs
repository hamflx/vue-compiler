pub(crate) fn resolve_vue3_type_import_path(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let extension = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let stem = candidate.with_extension("");
    let mut candidates = Vec::new();
    if !extension.is_empty() {
        if !matches!(
            extension,
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
        ) {
            candidates.push(arbitrary_extension_type_candidate(&stem, extension));
        }
        if matches!(extension, "js" | "jsx" | "mjs" | "cjs") {
            candidates.extend(vue3_ts_resolution_candidates(&stem, extension));
        }
        candidates.push(candidate.to_path_buf());
    } else {
        if candidate.is_dir() {
            match resolve_vue3_package_json_type_entry(candidate, None, type_resolver) {
                Vue3PackageJsonTypeResolution::Resolved(path) => return Some(path),
                Vue3PackageJsonTypeResolution::Blocked => return None,
                Vue3PackageJsonTypeResolution::NoPackageJson
                | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {}
            }
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
        }
        candidates.extend(vue3_ts_resolution_candidates(candidate, extension));
        candidates.push(candidate.join("index.ts"));
        candidates.push(candidate.join("index.tsx"));
        candidates.push(candidate.join("index.d.ts"));
    }
    candidates.into_iter().find(|candidate| candidate.exists())
}

pub(crate) fn arbitrary_extension_type_candidate(stem: &Path, extension: &str) -> PathBuf {
    let Some(file_name) = stem.file_name().and_then(|name| name.to_str()) else {
        return stem.with_extension(format!("d.{extension}.ts"));
    };
    let mut candidate = stem.to_path_buf();
    candidate.set_file_name(format!("{file_name}.d.{extension}.ts"));
    candidate
}

pub(crate) fn vue3_ts_resolution_candidates(base: &Path, extension: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if extension == "mjs" {
        candidates.push(path_with_extension(base, "mts"));
        candidates.push(path_with_extension(base, "d.mts"));
    } else if extension == "cjs" {
        candidates.push(path_with_extension(base, "cts"));
        candidates.push(path_with_extension(base, "d.cts"));
    }
    candidates.push(path_with_extension(base, "ts"));
    candidates.push(path_with_extension(base, "tsx"));
    candidates.push(path_with_extension(base, "d.ts"));
    if extension.is_empty() {
        candidates.push(path_with_extension(base, "mts"));
        candidates.push(path_with_extension(base, "d.mts"));
        candidates.push(path_with_extension(base, "cts"));
        candidates.push(path_with_extension(base, "d.cts"));
    }
    candidates
}

pub(crate) fn path_with_extension(path: &Path, extension: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension(extension);
    path
}

pub(crate) fn vue3_bounded_replace(
    source: &str,
    pattern: &str,
    replacement: &str,
    max_bytes: usize,
) -> Option<String> {
    if pattern.is_empty() {
        return None;
    }
    let matches = source.match_indices(pattern).count();
    let removed = matches.checked_mul(pattern.len())?;
    let added = matches.checked_mul(replacement.len())?;
    let output_len = source.len().checked_sub(removed)?.checked_add(added)?;
    if output_len > max_bytes {
        return None;
    }
    let mut output = String::with_capacity(output_len);
    let mut remainder = source;
    while let Some(index) = remainder.find(pattern) {
        output.push_str(&remainder[..index]);
        output.push_str(replacement);
        remainder = &remainder[index + pattern.len()..];
    }
    output.push_str(remainder);
    debug_assert_eq!(output.len(), output_len);
    Some(output)
}

pub(crate) fn normalize_path_components(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
