pub(crate) fn resolve_vue3_type_import_path(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        type_resolver,
        Vue3TypeImportPathProbeMode::Source,
    )
}

pub(crate) fn resolve_vue3_type_reference_path(
    filename: &str,
    reference: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    match type_resolver
        .external_type_session
        .begin_type_import_resolution(
            Vue3TypeResolutionKind::ReferencePath,
            filename,
            reference,
            &type_resolver.typescript_version,
            true,
        )
    {
        Vue3TypeImportResolutionLoad::Ready(resolution) => resolution,
        Vue3TypeImportResolutionLoad::Failed => None,
        Vue3TypeImportResolutionLoad::Start {
            cache_key,
            failure_epoch,
        } => {
            let resolution =
                resolve_vue3_type_reference_path_uncached(filename, reference, type_resolver);
            type_resolver
                .external_type_session
                .finish_type_import_resolution(cache_key, resolution, failure_epoch, true)
        }
    }
}

fn resolve_vue3_type_reference_path_uncached(
    filename: &str,
    reference: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if reference.is_empty() {
        return None;
    }
    let base = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let candidate = normalize_path_components(base.join(reference.replace('\\', "/")));
    let max_path_bytes = type_resolver
        .external_type_session
        .limits()
        .max_generated_path_bytes
        .min(VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES);
    if candidate.as_os_str().as_encoded_bytes().len() > max_path_bytes {
        type_resolver
            .external_type_session
            .record_resolution_failure();
        return None;
    }
    if candidate.extension().is_some() {
        if !vue3_type_reference_path_has_supported_extension(&candidate) {
            return None;
        }
        return candidate.is_file().then_some(candidate);
    }
    for extension in ["ts", "tsx", "d.ts"] {
        let candidate = path_with_extension(&candidate, extension);
        if candidate.as_os_str().as_encoded_bytes().len() > max_path_bytes {
            type_resolver
                .external_type_session
                .record_resolution_failure();
            return None;
        }
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn vue3_type_reference_path_has_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ts", "tsx", "mts", "cts", "vue"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

pub(crate) fn resolve_vue3_metadata_type_reference_declaration_file(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_reference_declaration_file_with_probe_mode(
        candidate,
        type_resolver,
        Vue3TypeImportPathProbeMode::Metadata,
    )
}

fn resolve_vue3_type_reference_declaration_file_with_probe_mode(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
    probe_mode: Vue3TypeImportPathProbeMode,
) -> Option<PathBuf> {
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    for candidate in vue3_type_reference_declaration_candidates(candidate) {
        if !probe_mode.path_is_within_limit(&candidate, type_resolver) {
            return None;
        }
        if probe_mode.exists(&candidate, type_resolver)? {
            return Some(candidate);
        }
    }
    None
}

fn vue3_type_reference_declaration_candidates(candidate: &Path) -> Vec<PathBuf> {
    let file_name = candidate
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|extension| file_name.ends_with(extension))
    {
        return vec![candidate.to_path_buf()];
    }
    let extension = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if extension.is_empty() {
        return vec![path_with_extension(candidate, "d.ts")];
    }
    let declaration_extension = match extension {
        "mts" | "mjs" => Some("d.mts"),
        "cts" | "cjs" => Some("d.cts"),
        "ts" | "tsx" | "js" | "jsx" => Some("d.ts"),
        _ => None,
    };
    let mut candidates = Vec::new();
    if let Some(declaration_extension) = declaration_extension {
        let stem = candidate.with_extension("");
        candidates.push(path_with_extension(&stem, declaration_extension));
    } else {
        candidates.push(arbitrary_extension_type_candidate(
            &candidate.with_extension(""),
            extension,
        ));
    }
    if !file_name.is_empty() {
        let mut appended = candidate.to_path_buf();
        appended.set_file_name(format!("{file_name}.d.ts"));
        if candidates.first() != Some(&appended) {
            candidates.push(appended);
        }
    }
    candidates
}

pub(crate) fn resolve_vue3_metadata_type_import_path(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        type_resolver,
        Vue3TypeImportPathProbeMode::Metadata,
    )
}

#[derive(Clone, Copy)]
enum Vue3TypeImportPathProbeMode {
    Source,
    Metadata,
}

impl Vue3TypeImportPathProbeMode {
    fn path_is_within_limit(
        self,
        path: &Path,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        let max_path_bytes = type_resolver
            .external_type_session
            .limits()
            .max_generated_path_bytes
            .min(VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES);
        if path.as_os_str().as_encoded_bytes().len() <= max_path_bytes {
            return true;
        }
        match self {
            Self::Source => type_resolver
                .external_type_session
                .record_resolution_failure(),
            Self::Metadata => type_resolver.external_type_session.block_metadata(),
        }
        false
    }

    fn is_dir(self, path: &Path, type_resolver: &Vue3TypeResolverContext) -> Option<bool> {
        match self {
            Self::Source => Some(path.is_dir()),
            Self::Metadata => type_resolver
                .external_type_session
                .metadata_path_is_dir(path),
        }
    }

    fn exists(self, path: &Path, type_resolver: &Vue3TypeResolverContext) -> Option<bool> {
        match self {
            Self::Source => Some(path.exists()),
            Self::Metadata => type_resolver
                .external_type_session
                .metadata_path_exists(path),
        }
    }
}

fn resolve_vue3_type_import_path_with_probe_mode(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
    probe_mode: Vue3TypeImportPathProbeMode,
) -> Option<PathBuf> {
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
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
        if probe_mode.is_dir(candidate, type_resolver)? {
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
    for candidate in candidates {
        if !probe_mode.path_is_within_limit(&candidate, type_resolver) {
            return None;
        }
        if probe_mode.exists(&candidate, type_resolver)? {
            return Some(candidate);
        }
    }
    None
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
                let can_pop = matches!(
                    normalized.components().next_back(),
                    Some(std::path::Component::Normal(_))
                );
                if can_pop {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
