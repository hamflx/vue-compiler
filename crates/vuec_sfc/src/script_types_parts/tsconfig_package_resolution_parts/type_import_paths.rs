pub(crate) fn resolve_vue3_type_import_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        resolution_mode,
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
            type_resolver,
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
    if vue3_typescript_path_extension(&candidate).is_some() {
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
    vue3_typescript_path_extension(path).is_some_and(|extension| {
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
    resolve_vue3_module_suffixed_file(
        vue3_type_reference_declaration_candidates(candidate),
        type_resolver,
        probe_mode,
    )
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
    let Some(extension) = vue3_typescript_path_extension(candidate) else {
        return vec![path_with_extension(candidate, "d.ts")];
    };
    let declaration_extension = match extension {
        "mts" | "mjs" => Some("d.mts"),
        "cts" | "cjs" => Some("d.cts"),
        "ts" | "tsx" | "js" | "jsx" => Some("d.ts"),
        _ => None,
    };
    let mut candidates = Vec::new();
    if let Some(declaration_extension) = declaration_extension {
        candidates.push(vue3_path_with_typescript_extension(
            candidate,
            declaration_extension,
        ));
    } else {
        candidates.push(arbitrary_extension_type_candidate(candidate, extension));
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

pub(crate) fn resolve_vue3_metadata_type_import_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        resolution_mode,
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
        self.path_bytes_are_within_limit(
            path.as_os_str().as_encoded_bytes().len(),
            type_resolver,
        )
    }

    fn suffixed_path_is_within_limit(
        self,
        path: &Path,
        suffix: &str,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        let Some(path_bytes) = path
            .as_os_str()
            .as_encoded_bytes()
            .len()
            .checked_add(suffix.len())
        else {
            return self.path_bytes_are_within_limit(usize::MAX, type_resolver);
        };
        self.path_bytes_are_within_limit(path_bytes, type_resolver)
    }

    fn path_bytes_are_within_limit(
        self,
        path_bytes: usize,
        type_resolver: &Vue3TypeResolverContext,
    ) -> bool {
        let max_path_bytes = type_resolver
            .external_type_session
            .limits()
            .max_generated_path_bytes
            .min(VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES);
        if path_bytes <= max_path_bytes {
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
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    probe_mode: Vue3TypeImportPathProbeMode,
) -> Option<PathBuf> {
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    let extension = vue3_typescript_path_extension(candidate);
    let mut candidates = Vec::new();
    if let Some(extension) = extension {
        if !matches!(
            extension,
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
        ) {
            candidates.push(arbitrary_extension_type_candidate(candidate, extension));
        }
        if matches!(extension, "js" | "jsx" | "mjs" | "cjs") {
            candidates.extend(vue3_ts_resolution_candidates(candidate, Some(extension)));
        }
        candidates.push(candidate.to_path_buf());
        return resolve_vue3_module_suffixed_file(candidates, type_resolver, probe_mode);
    }
    let failure_epoch = type_resolver.external_type_session.failure_epoch();
    let resolved = resolve_vue3_module_suffixed_file(
        vue3_ts_resolution_candidates(candidate, None),
        type_resolver,
        probe_mode,
    );
    if resolved.is_some()
        || type_resolver.external_type_session.failure_epoch() != failure_epoch
    {
        return resolved;
    }
    if probe_mode.is_dir(candidate, type_resolver)? {
        match resolve_vue3_package_json_type_entry_with_mode(
            candidate,
            None,
            resolution_mode,
            type_resolver,
        ) {
            Vue3PackageJsonTypeResolution::Resolved(path) => return Some(path),
            Vue3PackageJsonTypeResolution::Blocked => return None,
            Vue3PackageJsonTypeResolution::NoPackageJson
            | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {}
        }
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
    }
    resolve_vue3_module_suffixed_file(
        [
            candidate.join("index.ts"),
            candidate.join("index.tsx"),
            candidate.join("index.d.ts"),
        ],
        type_resolver,
        probe_mode,
    )
}

fn resolve_vue3_module_suffixed_file(
    candidates: impl IntoIterator<Item = PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    probe_mode: Vue3TypeImportPathProbeMode,
) -> Option<PathBuf> {
    for candidate in candidates {
        if !probe_mode.path_is_within_limit(&candidate, type_resolver) {
            return None;
        }
        for suffix in type_resolver.module_suffixes.iter() {
            if !probe_mode.suffixed_path_is_within_limit(&candidate, suffix, type_resolver) {
                return None;
            }
            let candidate = vue3_path_with_module_suffix(&candidate, suffix);
            if probe_mode.exists(&candidate, type_resolver)? {
                return Some(candidate);
            }
        }
    }
    None
}

fn vue3_path_with_module_suffix(path: &Path, suffix: &str) -> PathBuf {
    if suffix.is_empty() {
        return path.to_path_buf();
    }
    let Some(file_name) = path.file_name() else {
        let mut suffixed = path.as_os_str().to_os_string();
        suffixed.push(suffix);
        return PathBuf::from(suffixed);
    };
    let Some(file_name_text) = file_name.to_str() else {
        let mut file_name = file_name.to_os_string();
        file_name.push(suffix);
        let mut suffixed = path.to_path_buf();
        suffixed.set_file_name(file_name);
        return suffixed;
    };
    const EXTENSIONS: [&str; 12] = [
        ".d.ts", ".d.mts", ".d.cts", ".mjs", ".mts", ".cjs", ".cts", ".ts", ".js",
        ".tsx", ".jsx", ".json",
    ];
    let extension = EXTENSIONS
        .into_iter()
        .find(|extension| file_name_text.ends_with(extension))
        .unwrap_or_default();
    let stem = &file_name_text[..file_name_text.len() - extension.len()];
    let mut suffixed = path.to_path_buf();
    suffixed.set_file_name(format!("{stem}{suffix}{extension}"));
    suffixed
}

pub(crate) fn arbitrary_extension_type_candidate(candidate: &Path, extension: &str) -> PathBuf {
    vue3_path_with_typescript_extension(candidate, &format!("d.{extension}.ts"))
}

pub(crate) fn vue3_ts_resolution_candidates(
    candidate: &Path,
    extension: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if extension == Some("mjs") {
        candidates.push(vue3_path_with_typescript_extension(candidate, "mts"));
        candidates.push(vue3_path_with_typescript_extension(candidate, "d.mts"));
    } else if extension == Some("cjs") {
        candidates.push(vue3_path_with_typescript_extension(candidate, "cts"));
        candidates.push(vue3_path_with_typescript_extension(candidate, "d.cts"));
    }
    candidates.push(vue3_path_with_typescript_extension(candidate, "ts"));
    candidates.push(vue3_path_with_typescript_extension(candidate, "tsx"));
    candidates.push(vue3_path_with_typescript_extension(candidate, "d.ts"));
    if extension.is_none() {
        candidates.push(vue3_path_with_typescript_extension(candidate, "mts"));
        candidates.push(vue3_path_with_typescript_extension(candidate, "d.mts"));
        candidates.push(vue3_path_with_typescript_extension(candidate, "cts"));
        candidates.push(vue3_path_with_typescript_extension(candidate, "d.cts"));
    }
    candidates
}

fn vue3_typescript_path_extension(path: &Path) -> Option<&str> {
    let file_name = path.file_name()?.to_str()?;
    let dot = file_name.rfind('.')?;
    Some(&file_name[dot + 1..])
}

fn vue3_path_with_typescript_extension(path: &Path, extension: &str) -> PathBuf {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return path_with_extension(path, extension);
    };
    let stem = file_name
        .rfind('.')
        .map_or(file_name, |dot| &file_name[..dot]);
    let mut candidate = path.to_path_buf();
    candidate.set_file_name(format!("{stem}.{extension}"));
    candidate
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

pub(crate) fn vue3_bounded_replace_first(
    source: &str,
    pattern: &str,
    replacement: &str,
    max_bytes: usize,
) -> Option<String> {
    if pattern.is_empty() {
        return None;
    }
    let Some(index) = source.find(pattern) else {
        return (source.len() <= max_bytes).then(|| source.to_string());
    };
    let suffix_index = index.checked_add(pattern.len())?;
    let output_len = source
        .len()
        .checked_sub(pattern.len())?
        .checked_add(replacement.len())?;
    if output_len > max_bytes {
        return None;
    }
    let mut output = String::with_capacity(output_len);
    output.push_str(&source[..index]);
    output.push_str(replacement);
    output.push_str(&source[suffix_index..]);
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
