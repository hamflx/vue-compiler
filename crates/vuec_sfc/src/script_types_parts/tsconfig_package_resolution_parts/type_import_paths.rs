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
        Vue3TypeImportPathSemantics::ModuleSpecifier,
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
            None,
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
        return type_resolver
            .external_type_session
            .source_resolution_path_is_file(&candidate)?
            .then_some(candidate);
    }
    for extension in ["ts", "tsx", "d.ts"] {
        let candidate = path_with_extension(&candidate, extension);
        if candidate.as_os_str().as_encoded_bytes().len() > max_path_bytes {
            type_resolver
                .external_type_session
                .record_resolution_failure();
            return None;
        }
        if type_resolver
            .external_type_session
            .source_resolution_path_is_file(&candidate)?
        {
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

pub(crate) fn resolve_vue3_metadata_package_target_declaration_file(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let probe_mode = Vue3TypeImportPathProbeMode::Metadata;
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    let candidate = vue3_package_target_declaration_candidate(candidate)?;
    resolve_vue3_module_suffixed_file([candidate], type_resolver, probe_mode)
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
    if vue3_typescript_path_extension(candidate).is_none() {
        return vec![path_with_extension(candidate, "d.ts")];
    }
    let Some(primary) = vue3_package_target_declaration_candidate(candidate) else {
        return Vec::new();
    };
    let mut candidates = vec![primary];
    if !file_name.is_empty() {
        let mut appended = candidate.to_path_buf();
        appended.set_file_name(format!("{file_name}.d.ts"));
        if candidates.first() != Some(&appended) {
            candidates.push(appended);
        }
    }
    candidates
}

fn vue3_package_target_declaration_candidate(candidate: &Path) -> Option<PathBuf> {
    let file_name = candidate.file_name()?.to_str()?;
    if [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|extension| file_name.ends_with(extension))
    {
        return Some(candidate.to_path_buf());
    }
    let extension = vue3_typescript_path_extension(candidate)?;
    let declaration_extension = match extension {
        "mts" | "mjs" => "d.mts",
        "cts" | "cjs" => "d.cts",
        "ts" | "tsx" | "js" | "jsx" => "d.ts",
        extension => return Some(arbitrary_extension_type_candidate(candidate, extension)),
    };
    Some(vue3_path_with_typescript_extension(
        candidate,
        declaration_extension,
    ))
}

pub(crate) fn resolve_vue3_metadata_module_specifier_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        resolution_mode,
        type_resolver,
        Vue3TypeImportPathProbeMode::Metadata,
        Vue3TypeImportPathSemantics::ModuleSpecifier,
    )
}

#[cfg(test)]
pub(crate) fn resolve_vue3_metadata_package_map_target_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        resolution_mode,
        type_resolver,
        Vue3TypeImportPathProbeMode::Metadata,
        Vue3TypeImportPathSemantics::PackageMapTarget,
    )
}

pub(crate) fn resolve_vue3_metadata_package_map_type_target_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        resolution_mode,
        type_resolver,
        Vue3TypeImportPathProbeMode::Metadata,
        Vue3TypeImportPathSemantics::PackageMapTypeTarget,
    )
}

pub(crate) fn resolve_vue3_metadata_legacy_package_type_field_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    policy: Vue3PackageTargetPathPolicy,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    resolve_vue3_type_import_path_with_probe_mode(
        candidate,
        resolution_mode,
        type_resolver,
        Vue3TypeImportPathProbeMode::Metadata,
        Vue3TypeImportPathSemantics::LegacyPackageTypeField(policy),
    )
}

pub(crate) fn resolve_vue3_metadata_types_versions_type_target_path_with_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    policy: Vue3PackageTargetPathPolicy,
    try_raw_target: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if try_raw_target {
        let resolved = resolve_vue3_metadata_types_versions_raw_target_path(
            candidate,
            type_resolver,
        );
        if resolved.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return resolved;
        }
    }
    resolve_vue3_metadata_legacy_package_type_field_path_with_mode(
        candidate,
        resolution_mode,
        policy,
        type_resolver,
    )
}

pub(crate) fn resolve_vue3_metadata_legacy_package_javascript_field_path(
    candidate: &Path,
    policy: Vue3PackageTargetPathPolicy,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let probe_mode = Vue3TypeImportPathProbeMode::Metadata;
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    let failure_epoch = type_resolver.external_type_session.failure_epoch();
    let mut candidates = vue3_javascript_package_field_replacement_candidates(candidate);
    if policy == Vue3PackageTargetPathPolicy::AllowImplicit {
        candidates.extend(vue3_javascript_appended_resolution_candidates(candidate));
    }
    let resolved = resolve_vue3_module_suffixed_file(candidates, type_resolver, probe_mode);
    if resolved.is_some()
        || policy == Vue3PackageTargetPathPolicy::RequireExplicitFileName
        || type_resolver.external_type_session.failure_epoch() != failure_epoch
    {
        return resolved;
    }
    resolve_vue3_module_suffixed_file(
        [candidate.join("index.js"), candidate.join("index.jsx")],
        type_resolver,
        probe_mode,
    )
}

pub(crate) fn resolve_vue3_metadata_types_versions_javascript_target_path(
    candidate: &Path,
    policy: Vue3PackageTargetPathPolicy,
    try_raw_target: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if try_raw_target {
        let resolved = resolve_vue3_metadata_types_versions_raw_target_path(
            candidate,
            type_resolver,
        );
        if resolved.is_some() || type_resolver.external_type_session.metadata_is_blocked() {
            return resolved;
        }
    }
    resolve_vue3_metadata_legacy_package_javascript_field_path(
        candidate,
        policy,
        type_resolver,
    )
}

fn resolve_vue3_metadata_types_versions_raw_target_path(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let probe_mode = Vue3TypeImportPathProbeMode::Metadata;
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    resolve_vue3_module_suffixed_file(
        [candidate.to_path_buf()],
        type_resolver,
        probe_mode,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Vue3PackageTargetPathPolicy {
    AllowImplicit,
    RequireExplicitFileName,
}

pub(crate) fn resolve_vue3_metadata_bare_package_type_fallback_path_with_mode(
    candidate: &Path,
    root_package_dir: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    allow_package_manifest: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let probe_mode = Vue3TypeImportPathProbeMode::Metadata;
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    let uses_node_esm_specifier_rules = type_resolver
        .module_resolution
        .uses_node_esm_specifier_rules(resolution_mode, &type_resolver.typescript_version);
    if let Some(extension) = vue3_typescript_path_extension(candidate) {
        let mut candidates = Vec::new();
        let has_supported_source_extension = matches!(
            extension,
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
        );
        if !has_supported_source_extension {
            candidates.push(arbitrary_extension_type_candidate(candidate, extension));
            if !uses_node_esm_specifier_rules {
                candidates.extend(vue3_ts_appended_resolution_candidates(candidate));
            }
        }
        if matches!(extension, "js" | "jsx" | "mjs" | "cjs") {
            candidates.extend(vue3_ts_resolution_candidates(
                candidate,
                Some(extension),
                !uses_node_esm_specifier_rules,
            ));
        }
        if matches!(extension, "ts" | "tsx" | "mts" | "cts") {
            candidates.push(candidate.to_path_buf());
        }
        return resolve_vue3_module_suffixed_file(candidates, type_resolver, probe_mode);
    }
    let failure_epoch = type_resolver.external_type_session.failure_epoch();
    let resolved = if uses_node_esm_specifier_rules {
        None
    } else {
        resolve_vue3_module_suffixed_file(
            vue3_ts_resolution_candidates(candidate, None, true),
            type_resolver,
            probe_mode,
        )
    };
    if resolved.is_some()
        || type_resolver.external_type_session.failure_epoch() != failure_epoch
    {
        return resolved;
    }
    if probe_mode.is_dir(candidate, type_resolver)? {
        match resolve_vue3_package_subpath_index_types_versions_phase(
            root_package_dir,
            candidate,
            resolution_mode,
            Vue3PackageResolutionPhase::Types,
            type_resolver,
        ) {
            Vue3TypesVersionsResolution::Resolved(path) => return Some(path),
            Vue3TypesVersionsResolution::MatchedButMissing
            | Vue3TypesVersionsResolution::Blocked => return None,
            Vue3TypesVersionsResolution::NotMatched => {}
        }
        if allow_package_manifest {
            match resolve_vue3_package_json_directory_entry_phase_with_mode(
                candidate,
                resolution_mode,
                Vue3PackageResolutionPhase::Types,
                type_resolver,
            ) {
                Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
                Vue3PackageJsonPhaseResolution::Blocked => return None,
                Vue3PackageJsonPhaseResolution::Missing(fallback) if !fallback.allowed => {
                    return None;
                }
                Vue3PackageJsonPhaseResolution::NoPackageJson
                | Vue3PackageJsonPhaseResolution::Missing(_) => {}
            }
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
        }
    }
    if uses_node_esm_specifier_rules {
        return None;
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

pub(crate) fn resolve_vue3_metadata_bare_package_javascript_fallback_path_with_mode(
    candidate: &Path,
    root_package_dir: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    allow_package_manifest: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let probe_mode = Vue3TypeImportPathProbeMode::Metadata;
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    let uses_node_esm_specifier_rules = type_resolver
        .module_resolution
        .uses_node_esm_specifier_rules(resolution_mode, &type_resolver.typescript_version);
    let failure_epoch = type_resolver.external_type_session.failure_epoch();
    let mut candidates = vue3_javascript_package_field_replacement_candidates(candidate);
    if !uses_node_esm_specifier_rules {
        candidates.extend(vue3_javascript_appended_resolution_candidates(candidate));
    }
    let resolved = resolve_vue3_module_suffixed_file(candidates, type_resolver, probe_mode);
    if resolved.is_some()
        || type_resolver.external_type_session.failure_epoch() != failure_epoch
    {
        return resolved;
    }
    if probe_mode.is_dir(candidate, type_resolver)? {
        match resolve_vue3_package_subpath_index_types_versions_phase(
            root_package_dir,
            candidate,
            resolution_mode,
            Vue3PackageResolutionPhase::JavaScript,
            type_resolver,
        ) {
            Vue3TypesVersionsResolution::Resolved(path) => return Some(path),
            Vue3TypesVersionsResolution::MatchedButMissing
            | Vue3TypesVersionsResolution::Blocked => return None,
            Vue3TypesVersionsResolution::NotMatched => {}
        }
        if allow_package_manifest {
            match resolve_vue3_package_json_directory_entry_phase_with_mode(
                candidate,
                resolution_mode,
                Vue3PackageResolutionPhase::JavaScript,
                type_resolver,
            ) {
                Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
                Vue3PackageJsonPhaseResolution::Blocked => return None,
                Vue3PackageJsonPhaseResolution::Missing(fallback) if !fallback.allowed => {
                    return None;
                }
                Vue3PackageJsonPhaseResolution::NoPackageJson
                | Vue3PackageJsonPhaseResolution::Missing(_) => {}
            }
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
        }
    }
    if uses_node_esm_specifier_rules {
        return None;
    }
    resolve_vue3_module_suffixed_file(
        [candidate.join("index.js"), candidate.join("index.jsx")],
        type_resolver,
        probe_mode,
    )
}

#[derive(Clone, Copy)]
enum Vue3TypeImportPathProbeMode {
    Source,
    Metadata,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Vue3TypeImportPathSemantics {
    ModuleSpecifier,
    #[cfg(test)]
    PackageMapTarget,
    PackageMapTypeTarget,
    LegacyPackageTypeField(Vue3PackageTargetPathPolicy),
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
            Self::Source => type_resolver
                .external_type_session
                .source_resolution_path_is_dir(path),
            Self::Metadata => type_resolver
                .external_type_session
                .metadata_path_is_dir(path),
        }
    }

    fn exists(self, path: &Path, type_resolver: &Vue3TypeResolverContext) -> Option<bool> {
        match self {
            Self::Source => type_resolver
                .external_type_session
                .source_resolution_path_exists(path),
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
    semantics: Vue3TypeImportPathSemantics,
) -> Option<PathBuf> {
    if !probe_mode.path_is_within_limit(candidate, type_resolver) {
        return None;
    }
    let uses_node_esm_specifier_rules = !matches!(
        semantics,
        Vue3TypeImportPathSemantics::LegacyPackageTypeField(_)
    )
        && type_resolver
            .module_resolution
            .uses_node_esm_specifier_rules(resolution_mode, &type_resolver.typescript_version);
    let requires_explicit_package_target = match semantics {
        #[cfg(test)]
        Vue3TypeImportPathSemantics::PackageMapTarget => true,
        Vue3TypeImportPathSemantics::PackageMapTypeTarget
        | Vue3TypeImportPathSemantics::LegacyPackageTypeField(
            Vue3PackageTargetPathPolicy::RequireExplicitFileName,
        ) => true,
        Vue3TypeImportPathSemantics::ModuleSpecifier
        | Vue3TypeImportPathSemantics::LegacyPackageTypeField(
            Vue3PackageTargetPathPolicy::AllowImplicit,
        ) => false,
    };
    let allows_appended_extensions =
        !requires_explicit_package_target && !uses_node_esm_specifier_rules;
    let uses_classic_specifier_rules = semantics
        == Vue3TypeImportPathSemantics::ModuleSpecifier
        && type_resolver.module_resolution == Vue3TypeModuleResolutionKind::Classic;
    let extension = vue3_typescript_path_extension(candidate);
    let mut candidates = Vec::new();
    if let Some(extension) = extension {
        if matches!(
            semantics,
            Vue3TypeImportPathSemantics::LegacyPackageTypeField(_)
        ) {
            return resolve_vue3_module_suffixed_file(
                vue3_package_type_field_resolution_candidates(
                    candidate,
                    allows_appended_extensions,
                ),
                type_resolver,
                probe_mode,
            );
        }
        let has_supported_source_extension = matches!(
            extension,
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
        );
        if !has_supported_source_extension {
            candidates.push(arbitrary_extension_type_candidate(candidate, extension));
            if allows_appended_extensions {
                candidates.extend(vue3_ts_appended_resolution_candidates(candidate));
            }
        }
        if matches!(extension, "js" | "jsx" | "mjs" | "cjs") {
            candidates.extend(vue3_ts_resolution_candidates(
                candidate,
                Some(extension),
                allows_appended_extensions,
            ));
        }
        let allows_candidate_as_written = match semantics {
            #[cfg(test)]
            Vue3TypeImportPathSemantics::PackageMapTarget => {
                has_supported_source_extension
            }
            Vue3TypeImportPathSemantics::PackageMapTypeTarget
            | Vue3TypeImportPathSemantics::LegacyPackageTypeField(_) => {
                matches!(extension, "ts" | "tsx" | "mts" | "cts")
            }
            Vue3TypeImportPathSemantics::ModuleSpecifier
            => true,
        };
        if allows_candidate_as_written {
            candidates.push(candidate.to_path_buf());
        }
        return resolve_vue3_module_suffixed_file(candidates, type_resolver, probe_mode);
    }
    if requires_explicit_package_target {
        return None;
    }
    if uses_node_esm_specifier_rules
        && semantics == Vue3TypeImportPathSemantics::ModuleSpecifier
    {
        return None;
    }
    let failure_epoch = type_resolver.external_type_session.failure_epoch();
    let resolved = if uses_node_esm_specifier_rules {
        None
    } else {
        resolve_vue3_module_suffixed_file(
            vue3_ts_resolution_candidates(candidate, None, true),
            type_resolver,
            probe_mode,
        )
    };
    if resolved.is_some()
        || uses_classic_specifier_rules
        || type_resolver.external_type_session.failure_epoch() != failure_epoch
    {
        return resolved;
    }
    let allows_directory_manifest = !matches!(
        semantics,
        Vue3TypeImportPathSemantics::LegacyPackageTypeField(_)
    );
    if allows_directory_manifest
        && probe_mode.is_dir(candidate, type_resolver)?
    {
        return resolve_vue3_module_specifier_directory_path_with_probe_mode(
            candidate,
            resolution_mode,
            type_resolver,
            probe_mode,
            uses_node_esm_specifier_rules,
        );
    }
    if uses_node_esm_specifier_rules {
        return None;
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

fn resolve_vue3_module_specifier_directory_path_with_probe_mode(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    probe_mode: Vue3TypeImportPathProbeMode,
    uses_node_esm_specifier_rules: bool,
) -> Option<PathBuf> {
    let type_fallback = match resolve_vue3_package_json_directory_entry_phase_with_mode(
        candidate,
        resolution_mode,
        Vue3PackageResolutionPhase::Types,
        type_resolver,
    ) {
        Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
        Vue3PackageJsonPhaseResolution::Blocked => return None,
        Vue3PackageJsonPhaseResolution::NoPackageJson => Vue3PackagePathFallback {
            allowed: true,
            allow_nested_manifest: true,
            allow_index: true,
        },
        Vue3PackageJsonPhaseResolution::Missing(fallback) => fallback,
    };
    if type_fallback.allowed && type_fallback.allow_index && !uses_node_esm_specifier_rules {
        let failure_epoch = type_resolver.external_type_session.failure_epoch();
        let resolved = resolve_vue3_module_suffixed_file(
            [
                candidate.join("index.ts"),
                candidate.join("index.tsx"),
                candidate.join("index.d.ts"),
            ],
            type_resolver,
            probe_mode,
        );
        if resolved.is_some()
            || type_resolver.external_type_session.failure_epoch() != failure_epoch
        {
            return resolved;
        }
    }

    let javascript_fallback = match resolve_vue3_package_json_directory_entry_phase_with_mode(
        candidate,
        resolution_mode,
        Vue3PackageResolutionPhase::JavaScript,
        type_resolver,
    ) {
        Vue3PackageJsonPhaseResolution::Resolved(path) => return Some(path),
        Vue3PackageJsonPhaseResolution::Blocked => return None,
        Vue3PackageJsonPhaseResolution::NoPackageJson => Vue3PackagePathFallback {
            allowed: true,
            allow_nested_manifest: true,
            allow_index: true,
        },
        Vue3PackageJsonPhaseResolution::Missing(fallback) => fallback,
    };
    if !javascript_fallback.allowed
        || !javascript_fallback.allow_index
        || uses_node_esm_specifier_rules
    {
        return None;
    }
    resolve_vue3_module_suffixed_file(
        [candidate.join("index.js"), candidate.join("index.jsx")],
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

fn vue3_javascript_package_field_replacement_candidates(candidate: &Path) -> Vec<PathBuf> {
    let Some(file_name) = candidate.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let lowercase = file_name.to_ascii_lowercase();
    let (suffix, replacements): (&str, &[&str]) = if lowercase.ends_with(".d.mts") {
        (".d.mts", &[".mjs"])
    } else if lowercase.ends_with(".d.cts") {
        (".d.cts", &[".cjs"])
    } else if lowercase.ends_with(".d.ts") {
        (".d.ts", &[".js", ".jsx"])
    } else if lowercase.ends_with(".mjs") || lowercase.ends_with(".mts") {
        (&file_name[file_name.len() - 4..], &[".mjs"])
    } else if lowercase.ends_with(".cjs") || lowercase.ends_with(".cts") {
        (&file_name[file_name.len() - 4..], &[".cjs"])
    } else if lowercase.ends_with(".jsx") || lowercase.ends_with(".tsx") {
        (&file_name[file_name.len() - 4..], &[".jsx", ".js"])
    } else if lowercase.ends_with(".js") || lowercase.ends_with(".ts") {
        (&file_name[file_name.len() - 3..], &[".js", ".jsx"])
    } else {
        return Vec::new();
    };
    let stem = &file_name[..file_name.len() - suffix.len()];
    replacements
        .iter()
        .map(|replacement| {
            let mut path = candidate.to_path_buf();
            path.set_file_name(format!("{stem}{replacement}"));
            path
        })
        .collect()
}

fn vue3_javascript_appended_resolution_candidates(candidate: &Path) -> [PathBuf; 2] {
    [
        vue3_path_with_appended_typescript_extension(candidate, "js"),
        vue3_path_with_appended_typescript_extension(candidate, "jsx"),
    ]
}

fn vue3_package_type_field_resolution_candidates(
    candidate: &Path,
    include_appended: bool,
) -> Vec<PathBuf> {
    let Some(file_name) = candidate.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let lowercase = file_name.to_ascii_lowercase();
    let mut candidates = Vec::new();
    let (suffix_len, replacements): (usize, &[&str]) = if lowercase.ends_with(".d.mts") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (6, &[".mts", ".d.mts"])
    } else if lowercase.ends_with(".d.cts") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (6, &[".cts", ".d.cts"])
    } else if lowercase.ends_with(".d.ts") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (5, &[".ts", ".tsx", ".d.ts"])
    } else if lowercase.ends_with(".mts") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (4, &[".mts", ".d.mts"])
    } else if lowercase.ends_with(".cts") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (4, &[".cts", ".d.cts"])
    } else if lowercase.ends_with(".tsx") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (4, &[".tsx", ".ts", ".d.ts"])
    } else if lowercase.ends_with(".ts") {
        push_unique_path(&mut candidates, candidate.to_path_buf());
        (3, &[".ts", ".tsx", ".d.ts"])
    } else if lowercase.ends_with(".mjs") {
        (4, &[".mts", ".d.mts"])
    } else if lowercase.ends_with(".cjs") {
        (4, &[".cts", ".d.cts"])
    } else if lowercase.ends_with(".jsx") {
        (4, &[".tsx", ".ts", ".d.ts"])
    } else if lowercase.ends_with(".js") {
        (3, &[".ts", ".tsx", ".d.ts"])
    } else {
        let Some(dot) = file_name.rfind('.') else {
            return candidates;
        };
        let extension = &file_name[dot + 1..];
        candidates.push(arbitrary_extension_type_candidate(candidate, extension));
        if include_appended {
            candidates.extend(vue3_ts_appended_resolution_candidates(candidate));
        }
        return candidates;
    };
    let stem = &file_name[..file_name.len() - suffix_len];
    for replacement in replacements {
        let mut path = candidate.to_path_buf();
        path.set_file_name(format!("{stem}{replacement}"));
        push_unique_path(&mut candidates, path);
    }
    if include_appended {
        for path in vue3_ts_appended_resolution_candidates(candidate) {
            push_unique_path(&mut candidates, path);
        }
    }
    candidates
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
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
    include_appended: bool,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    match extension {
        Some("mjs") => {
            candidates.push(vue3_path_with_typescript_extension(candidate, "mts"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "d.mts"));
        }
        Some("cjs") => {
            candidates.push(vue3_path_with_typescript_extension(candidate, "cts"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "d.cts"));
        }
        Some("jsx") => {
            candidates.push(vue3_path_with_typescript_extension(candidate, "tsx"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "ts"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "d.ts"));
        }
        Some("js") => {
            candidates.push(vue3_path_with_typescript_extension(candidate, "ts"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "tsx"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "d.ts"));
        }
        None => {
            candidates.push(vue3_path_with_typescript_extension(candidate, "ts"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "tsx"));
            candidates.push(vue3_path_with_typescript_extension(candidate, "d.ts"));
        }
        Some(_) => {}
    }
    if extension.is_some() && include_appended {
        candidates.extend(vue3_ts_appended_resolution_candidates(candidate));
    }
    candidates
}

fn vue3_ts_appended_resolution_candidates(candidate: &Path) -> [PathBuf; 3] {
    [
        vue3_path_with_appended_typescript_extension(candidate, "ts"),
        vue3_path_with_appended_typescript_extension(candidate, "tsx"),
        vue3_path_with_appended_typescript_extension(candidate, "d.ts"),
    ]
}

fn vue3_path_with_appended_typescript_extension(path: &Path, extension: &str) -> PathBuf {
    let appended_extension = format!(".{extension}");
    let Some(file_name) = path.file_name() else {
        let mut appended = path.as_os_str().to_os_string();
        appended.push(appended_extension);
        return PathBuf::from(appended);
    };
    let mut file_name = file_name.to_os_string();
    file_name.push(appended_extension);
    let mut candidate = path.to_path_buf();
    candidate.set_file_name(file_name);
    candidate
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

#[cfg(test)]
mod vue3_type_import_candidate_tests {
    use super::*;

    fn file_names(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<String> {
        candidates
            .into_iter()
            .map(|path| {
                path.file_name()
                    .expect("candidate file name")
                    .to_string_lossy()
                    .to_string()
            })
            .collect()
    }

    fn resolver_with_limits(limits: Vue3ExternalTypeLoadLimits) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    fn resolver_with_module_resolution(
        module_resolution: Vue3TypeModuleResolutionKind,
    ) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            module_resolution,
            ..Vue3TypeResolverContext::default()
        }
    }

    #[test]
    fn explicit_javascript_extensions_use_replacement_then_appended_candidates() {
        assert_eq!(
            file_names(vue3_ts_resolution_candidates(
                Path::new("entry.js"),
                Some("js"),
                true,
            )),
            [
                "entry.ts",
                "entry.tsx",
                "entry.d.ts",
                "entry.js.ts",
                "entry.js.tsx",
                "entry.js.d.ts",
            ]
        );
        assert_eq!(
            file_names(vue3_ts_resolution_candidates(
                Path::new("entry.jsx"),
                Some("jsx"),
                true,
            )),
            [
                "entry.tsx",
                "entry.ts",
                "entry.d.ts",
                "entry.jsx.ts",
                "entry.jsx.tsx",
                "entry.jsx.d.ts",
            ]
        );
        assert_eq!(
            file_names(vue3_ts_resolution_candidates(
                Path::new("entry.mjs"),
                Some("mjs"),
                true,
            )),
            [
                "entry.mts",
                "entry.d.mts",
                "entry.mjs.ts",
                "entry.mjs.tsx",
                "entry.mjs.d.ts",
            ]
        );
        assert_eq!(
            file_names(vue3_ts_resolution_candidates(
                Path::new("entry.cjs"),
                Some("cjs"),
                true,
            )),
            [
                "entry.cts",
                "entry.d.cts",
                "entry.cjs.ts",
                "entry.cjs.tsx",
                "entry.cjs.d.ts",
            ]
        );
    }

    #[test]
    fn legacy_package_fields_keep_type_and_javascript_candidate_phases_separate() {
        assert_eq!(
            file_names(vue3_package_type_field_resolution_candidates(
                Path::new("entry.js"),
                true,
            )),
            [
                "entry.ts",
                "entry.tsx",
                "entry.d.ts",
                "entry.js.ts",
                "entry.js.tsx",
                "entry.js.d.ts",
            ]
        );
        assert_eq!(
            file_names(vue3_package_type_field_resolution_candidates(
                Path::new("entry.ts"),
                false,
            )),
            ["entry.ts", "entry.tsx", "entry.d.ts"]
        );
        assert_eq!(
            file_names(vue3_package_type_field_resolution_candidates(
                Path::new("entry.d.mts"),
                false,
            )),
            ["entry.d.mts", "entry.mts"]
        );
        assert_eq!(
            file_names(vue3_javascript_package_field_replacement_candidates(
                Path::new("entry.ts"),
            )),
            ["entry.js", "entry.jsx"]
        );
        assert_eq!(
            file_names(vue3_javascript_package_field_replacement_candidates(
                Path::new("entry.jsx"),
            )),
            ["entry.jsx", "entry.js"]
        );
        assert_eq!(
            file_names(vue3_javascript_appended_resolution_candidates(
                Path::new("entry.css"),
            )),
            ["entry.css.js", "entry.css.jsx"]
        );
    }

    #[test]
    fn extensionless_candidates_do_not_probe_module_specific_extensions() {
        assert_eq!(
            file_names(vue3_ts_resolution_candidates(
                Path::new("entry"),
                None,
                true,
            )),
            ["entry.ts", "entry.tsx", "entry.d.ts"]
        );

        let dir = tempfile::tempdir().expect("temp dir");
        let resolver = Vue3TypeResolverContext::default();
        for (index, extension) in ["mts", "d.mts", "cts", "d.cts"].into_iter().enumerate() {
            let candidate = dir.path().join(format!("entry-{index}"));
            std::fs::write(candidate.with_extension(extension), "export interface Props {}")
                .expect("write module-specific target");
            assert!(resolve_vue3_type_import_path_with_mode(
                &candidate,
                Vue3TypeResolutionMode::Import,
                &resolver,
            )
            .is_none());
        }
    }

    #[test]
    fn directory_package_resolution_preserves_phase_order_and_matched_misses() {
        let dir = tempfile::tempdir().expect("temp dir");
        let matched_missing = dir.path().join("matched-missing");
        std::fs::create_dir_all(&matched_missing).expect("create matched package");
        std::fs::write(
            matched_missing.join("package.json"),
            r#"{"typesVersions":{"*":{"index":["missing.d.ts"]}}}"#,
        )
        .expect("write matched package manifest");
        std::fs::write(
            matched_missing.join("index.d.ts"),
            "export interface IndexDecoy {}",
        )
        .expect("write matched package index decoy");

        let type_index = dir.path().join("type-index");
        std::fs::create_dir_all(&type_index).expect("create type index package");
        std::fs::write(
            type_index.join("package.json"),
            r#"{"types":"missing.js","main":"main.js"}"#,
        )
        .expect("write type index package manifest");
        let index = type_index.join("index.d.ts");
        std::fs::write(&index, "export interface PreferredIndex {}")
            .expect("write preferred type index");
        std::fs::write(
            type_index.join("main.js"),
            "export const implementation = true;",
        )
        .expect("write JavaScript phase decoy");

        let resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &matched_missing,
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            None,
        );
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &type_index,
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            Some(index),
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn node_esm_module_specifiers_require_explicit_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let entry = dir.path().join("entry.ts");
        std::fs::write(&entry, "export interface EntryProps {}")
            .expect("write extensionless target");
        let package_dir = dir.path().join("package-dir");
        std::fs::create_dir_all(&package_dir).expect("create package directory");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"types":"types.d.ts","exports":{".":"./exports.d.ts"}}"#,
        )
        .expect("write directory package manifest");
        let package_entry = package_dir.join("types.d.ts");
        std::fs::write(&package_entry, "export interface PackageProps {}")
            .expect("write directory package entry");
        std::fs::write(
            package_dir.join("exports.d.ts"),
            "export interface WrongPackageProps {}",
        )
        .expect("write package exports decoy");
        let index_dir = dir.path().join("index-dir");
        std::fs::create_dir_all(&index_dir).expect("create index directory");
        let index_entry = index_dir.join("index.ts");
        std::fs::write(&index_entry, "export interface IndexProps {}")
            .expect("write directory index");
        let explicit = dir.path().join("explicit.ts");
        std::fs::write(&explicit, "export interface ExplicitProps {}")
            .expect("write explicit replacement");
        std::fs::write(
            dir.path().join("appended.js.d.ts"),
            "export interface AppendedProps {}",
        )
        .expect("write appended extension decoy");
        let arbitrary = dir.path().join("style.d.css.ts");
        std::fs::write(&arbitrary, "export interface StyleProps {}")
            .expect("write arbitrary extension declaration");
        std::fs::write(
            dir.path().join("legacy.css.d.ts"),
            "export interface LegacyProps {}",
        )
        .expect("write appended arbitrary decoy");

        let node_next =
            resolver_with_module_resolution(Vue3TypeModuleResolutionKind::NodeNext);
        for candidate in [
            dir.path().join("entry"),
            package_dir.clone(),
            index_dir.clone(),
            dir.path().join("appended.js"),
            dir.path().join("legacy.css"),
        ] {
            assert!(
                resolve_vue3_type_import_path_with_mode(
                    &candidate,
                    Vue3TypeResolutionMode::Import,
                    &node_next,
                )
                .is_none(),
                "unexpected NodeNext ESM resolution: {}",
                candidate.display()
            );
        }
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &dir.path().join("explicit.js"),
                Vue3TypeResolutionMode::Import,
                &node_next,
            ),
            Some(explicit)
        );
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &dir.path().join("style.css"),
                Vue3TypeResolutionMode::Import,
                &node_next,
            ),
            Some(arbitrary)
        );

        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &dir.path().join("entry"),
                Vue3TypeResolutionMode::Require,
                &node_next,
            ),
            Some(entry.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &package_dir,
                Vue3TypeResolutionMode::Require,
                &node_next,
            ),
            Some(package_entry)
        );
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &index_dir,
                Vue3TypeResolutionMode::Require,
                &node_next,
            ),
            Some(index_entry)
        );

        let bundler = resolver_with_module_resolution(Vue3TypeModuleResolutionKind::Bundler);
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &dir.path().join("entry"),
                Vue3TypeResolutionMode::Import,
                &bundler,
            ),
            Some(entry)
        );

        let legacy_package_target = dir.path().join("package-target.ts");
        std::fs::write(
            &legacy_package_target,
            "export interface PackageTargetProps {}",
        )
        .expect("write package target");
        assert_eq!(
            resolve_vue3_metadata_types_versions_type_target_path_with_mode(
                &dir.path().join("package-target"),
                Vue3TypeResolutionMode::Import,
                Vue3PackageTargetPathPolicy::AllowImplicit,
                false,
                &node_next,
            ),
            Some(legacy_package_target)
        );

        let suffixed = dir.path().join("platform.native.ts");
        std::fs::write(&suffixed, "export interface PlatformProps {}")
            .expect("write suffixed replacement");
        let suffixed_node_next = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            module_suffixes: std::sync::Arc::from([".native".to_string()]),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import_path_with_mode(
                &dir.path().join("platform.js"),
                Vue3TypeResolutionMode::Import,
                &suffixed_node_next,
            ),
            Some(suffixed)
        );
    }

    #[test]
    fn package_map_targets_require_explicit_file_names_in_every_mode() {
        let dir = tempfile::tempdir().expect("temp dir");
        let extensionless = dir.path().join("extensionless.ts");
        std::fs::write(&extensionless, "export interface ExtensionlessProps {}")
            .expect("write extensionless decoy");

        let directory = dir.path().join("directory");
        std::fs::create_dir_all(&directory).expect("create package target directory");
        std::fs::write(
            directory.join("package.json"),
            r#"{"types":"manifest.d.ts"}"#,
        )
        .expect("write nested package manifest");
        std::fs::write(
            directory.join("manifest.d.ts"),
            "export interface ManifestProps {}",
        )
        .expect("write nested manifest target");
        std::fs::write(
            directory.join("index.ts"),
            "export interface IndexProps {}",
        )
        .expect("write directory index decoy");

        let javascript = dir.path().join("javascript.d.ts");
        let module = dir.path().join("module.d.mts");
        let commonjs = dir.path().join("commonjs.d.cts");
        let arbitrary = dir.path().join("styles.d.css.ts");
        let declaration = dir.path().join("declaration.d.ts");
        for path in [&javascript, &module, &commonjs, &arbitrary, &declaration] {
            std::fs::write(path, "export interface Props {}")
                .expect("write explicit package map target");
        }
        std::fs::write(
            dir.path().join("appended.js.d.ts"),
            "export interface AppendedProps {}",
        )
        .expect("write appended extension decoy");
        std::fs::write(
            dir.path().join("raw.css"),
            "export interface RawProps {}",
        )
        .expect("write raw arbitrary extension decoy");

        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
            Vue3TypeModuleResolutionKind::Bundler,
        ] {
            let resolver = resolver_with_module_resolution(module_resolution);
            for resolution_mode in [
                Vue3TypeResolutionMode::Import,
                Vue3TypeResolutionMode::Require,
            ] {
                for candidate in [
                    dir.path().join("extensionless"),
                    directory.clone(),
                    dir.path().join("appended.js"),
                    dir.path().join("raw.css"),
                ] {
                    assert!(
                        resolve_vue3_metadata_package_map_target_path_with_mode(
                            &candidate,
                            resolution_mode,
                            &resolver,
                        )
                        .is_none(),
                        "unexpected {module_resolution:?} {resolution_mode:?} package-map resolution: {}",
                        candidate.display(),
                    );
                }
                for (candidate, expected) in [
                    (dir.path().join("javascript.js"), javascript.clone()),
                    (dir.path().join("module.mjs"), module.clone()),
                    (dir.path().join("commonjs.cjs"), commonjs.clone()),
                    (dir.path().join("styles.css"), arbitrary.clone()),
                    (declaration.clone(), declaration.clone()),
                ] {
                    assert_eq!(
                        resolve_vue3_metadata_package_map_target_path_with_mode(
                            &candidate,
                            resolution_mode,
                            &resolver,
                        ),
                        Some(expected),
                        "{module_resolution:?} {resolution_mode:?}: {}",
                        candidate.display(),
                    );
                }
            }
        }

        let legacy = resolver_with_module_resolution(Vue3TypeModuleResolutionKind::NodeNext);
        assert_eq!(
            resolve_vue3_metadata_types_versions_type_target_path_with_mode(
                &dir.path().join("extensionless"),
                Vue3TypeResolutionMode::Import,
                Vue3PackageTargetPathPolicy::AllowImplicit,
                false,
                &legacy,
            ),
            Some(extensionless),
        );
        assert_eq!(
            resolve_vue3_metadata_types_versions_type_target_path_with_mode(
                &directory,
                Vue3TypeResolutionMode::Import,
                Vue3PackageTargetPathPolicy::AllowImplicit,
                false,
                &legacy,
            ),
            Some(directory.join("index.ts")),
        );
    }

    #[test]
    fn appended_javascript_candidates_consume_exact_metadata_probe_budget() {
        let dir = tempfile::tempdir().expect("temp dir");
        let candidate = dir.path().join("entry.js");
        let target = dir.path().join("entry.js.d.ts");
        std::fs::write(&target, "export interface Props {}").expect("write appended target");
        let exact = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_resolution_path_probes: 6,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            resolve_vue3_metadata_module_specifier_path_with_mode(
                &candidate,
                Vue3TypeResolutionMode::Import,
                &exact,
            ),
            Some(target)
        );
        assert_eq!(
            exact
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            6
        );

        let short = resolver_with_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_resolution_path_probes: 5,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(resolve_vue3_metadata_module_specifier_path_with_mode(
            &candidate,
            Vue3TypeResolutionMode::Import,
            &short,
        )
        .is_none());
        assert_eq!(
            short
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            5
        );
        assert!(short.external_type_session.metadata_is_blocked());

        let strict_candidate = dir.path().join("strict.js");
        let strict_target = dir.path().join("strict.d.ts");
        std::fs::write(&strict_target, "export interface StrictProps {}")
            .expect("write strict replacement target");
        let strict_exact = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_metadata_resolution_path_probes: 3,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_metadata_module_specifier_path_with_mode(
                &strict_candidate,
                Vue3TypeResolutionMode::Import,
                &strict_exact,
            ),
            Some(strict_target)
        );
        assert_eq!(
            strict_exact
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            3
        );

        let strict_short = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_metadata_resolution_path_probes: 2,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
            ..Vue3TypeResolverContext::default()
        };
        assert!(resolve_vue3_metadata_module_specifier_path_with_mode(
            &strict_candidate,
            Vue3TypeResolutionMode::Import,
            &strict_short,
        )
        .is_none());
        assert_eq!(
            strict_short
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            2
        );
        assert!(strict_short.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn combined_module_suffix_paths_are_bounded_before_filesystem_probes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let candidate = dir.path().join("entry.ts");
        let target = dir.path().join("entry.native.ts");
        std::fs::write(&target, "export interface Props {}").expect("write suffixed target");
        let target_bytes = target.as_os_str().as_encoded_bytes().len();
        let exact = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([".native".to_string()]),
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_generated_path_bytes: target_bytes,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_metadata_module_specifier_path_with_mode(
                &candidate,
                Vue3TypeResolutionMode::Import,
                &exact,
            ),
            Some(target)
        );
        assert_eq!(
            exact
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            1
        );

        let short = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([".native".to_string()]),
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_generated_path_bytes: target_bytes - 1,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
            ..Vue3TypeResolverContext::default()
        };
        assert!(resolve_vue3_metadata_module_specifier_path_with_mode(
            &candidate,
            Vue3TypeResolutionMode::Import,
            &short,
        )
        .is_none());
        assert_eq!(
            short
                .external_type_session
                .stats()
                .metadata_resolution_path_probes,
            0
        );
        assert!(short.external_type_session.metadata_is_blocked());
    }
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
