#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PackageJsonTypeResolution {
    #[cfg(test)]
    NoPackageJson,
    #[cfg(test)]
    NoPackageTypeEntry,
    #[cfg(test)]
    NoPackageTypeEntryWithoutIndex,
    #[cfg(test)]
    NoPackageTypeEntryWithoutNestedManifest,
    Resolved(PathBuf),
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PackageResolutionPhase {
    Types,
    JavaScript,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Vue3PackagePathFallback {
    pub(crate) allowed: bool,
    pub(crate) allow_nested_manifest: bool,
    pub(crate) allow_index: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PackageJsonPhaseResolution {
    NoPackageJson,
    Missing(Vue3PackagePathFallback),
    Resolved(PathBuf),
    Blocked,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Vue3PackageJsonTypeManifest {
    pub(crate) name: Option<String>,
    pub(crate) version: Option<serde_json::Value>,
    pub(crate) tsconfig: Option<serde_json::Value>,
    pub(crate) exports: Option<serde_json::Value>,
    pub(crate) imports: Option<serde_json::Value>,
    pub(crate) types: Option<serde_json::Value>,
    pub(crate) typings: Option<serde_json::Value>,
    pub(crate) main: Option<serde_json::Value>,
    pub(crate) module_type: Vue3PackageModuleType,
    pub(crate) types_versions: Vue3PackageTypesVersions,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Vue3PackageModuleType {
    #[default]
    Unspecified,
    CommonJs,
    Module,
}

impl<'de> Deserialize<'de> for Vue3PackageJsonTypeManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PackageJsonManifestVisitor;

        impl<'de> Visitor<'de> for PackageJsonManifestVisitor {
            type Value = Vue3PackageJsonTypeManifest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package.json object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut manifest = Vue3PackageJsonTypeManifest::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => {
                            let value = map.next_value::<serde_json::Value>()?;
                            manifest.name = value.as_str().map(str::to_string);
                        }
                        "version" => manifest.version = Some(map.next_value()?),
                        "tsconfig" => manifest.tsconfig = Some(map.next_value()?),
                        "exports" => manifest.exports = Some(map.next_value()?),
                        "imports" => manifest.imports = Some(map.next_value()?),
                        "types" => manifest.types = Some(map.next_value()?),
                        "typings" => manifest.typings = Some(map.next_value()?),
                        "main" => manifest.main = Some(map.next_value()?),
                        "type" => {
                            let value = map.next_value::<serde_json::Value>()?;
                            manifest.module_type = match value.as_str() {
                                Some("module") => Vue3PackageModuleType::Module,
                                Some("commonjs") => Vue3PackageModuleType::CommonJs,
                                _ => Vue3PackageModuleType::Unspecified,
                            };
                        }
                        "typesVersions" => manifest.types_versions = map.next_value()?,
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(manifest)
            }
        }

        deserializer.deserialize_map(PackageJsonManifestVisitor)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Vue3PackageTypesVersions(Vec<Vue3PackageTypesVersionEntry>);

#[derive(Clone, Debug)]
pub(crate) struct Vue3PackageTypesVersionEntry {
    selector: String,
    value: Vue3PackageTypesVersionValue,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Vue3PackageTypesVersionMappings(Vec<(String, serde_json::Value)>);

#[derive(Clone, Debug)]
enum Vue3PackageTypesVersionValue {
    Unavailable,
    Mappings(Vue3PackageTypesVersionMappings),
}

fn vue3_insert_json_object_property<T>(
    properties: &mut Vec<(String, T)>,
    property_indexes: &mut BTreeMap<String, usize>,
    name: String,
    value: T,
) {
    if let Some(index) = property_indexes.get(&name).copied() {
        properties[index].1 = value;
        return;
    }
    property_indexes.insert(name.clone(), properties.len());
    properties.push((name, value));
}

fn vue3_sort_json_object_properties<T>(properties: &mut [(String, T)]) {
    // JSON.parse keeps the last duplicate value, then Object.keys enumerates array indexes first.
    properties.sort_by_key(|(name, _)| {
        vue3_javascript_array_index(name).map_or((1, 0), |index| (0, index))
    });
}

fn vue3_javascript_array_index(name: &str) -> Option<u32> {
    let index = name.parse::<u32>().ok()?;
    (index != u32::MAX && index.to_string() == name).then_some(index)
}

impl<'de> Deserialize<'de> for Vue3PackageTypesVersions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TypesVersionsVisitor;

        impl<'de> Visitor<'de> for TypesVersionsVisitor {
            type Value = Vue3PackageTypesVersions;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package.json typesVersions object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut properties = Vec::new();
                let mut property_indexes = BTreeMap::new();
                while let Some(selector) = map.next_key::<String>()? {
                    let value = map.next_value::<Vue3PackageTypesVersionValue>()?;
                    vue3_insert_json_object_property(
                        &mut properties,
                        &mut property_indexes,
                        selector,
                        value,
                    );
                }
                vue3_sort_json_object_properties(&mut properties);
                let entries = properties
                    .into_iter()
                    .map(|(selector, value)| Vue3PackageTypesVersionEntry { selector, value })
                    .collect();
                Ok(Vue3PackageTypesVersions(entries))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Vue3PackageTypesVersions::default())
            }
        }

        deserializer.deserialize_any(TypesVersionsVisitor)
    }
}

impl<'de> Deserialize<'de> for Vue3PackageTypesVersionValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TypesVersionMappingsVisitor;

        impl<'de> Visitor<'de> for TypesVersionMappingsVisitor {
            type Value = Vue3PackageTypesVersionValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package.json typesVersions mapping object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut mappings = Vec::new();
                let mut mapping_indexes = BTreeMap::new();
                while let Some(pattern) = map.next_key::<String>()? {
                    let target = map.next_value()?;
                    vue3_insert_json_object_property(
                        &mut mappings,
                        &mut mapping_indexes,
                        pattern,
                        target,
                    );
                }
                vue3_sort_json_object_properties(&mut mappings);
                Ok(Vue3PackageTypesVersionValue::Mappings(
                    Vue3PackageTypesVersionMappings(mappings),
                ))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Vue3PackageTypesVersionValue::Unavailable)
            }
        }

        deserializer.deserialize_any(TypesVersionMappingsVisitor)
    }
}

#[cfg(test)]
pub(crate) fn resolve_vue3_package_json_type_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    resolve_vue3_package_json_type_entry_with_exports(
        package_dir,
        subpath,
        type_resolver,
        Some(Vue3TypeResolutionMode::Import),
        false,
        true,
        true,
    )
}

#[cfg(test)]
pub(crate) fn resolve_vue3_package_json_type_entry_with_mode(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    resolve_vue3_package_json_type_entry_with_exports(
        package_dir,
        subpath,
        type_resolver,
        Some(resolution_mode),
        false,
        type_resolver.package_json_features().exports,
        true,
    )
}

#[cfg(test)]
pub(crate) fn resolve_vue3_package_json_type_reference_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    vue3_package_phase_resolution_to_type_resolution(
        resolve_vue3_package_json_type_reference_entry_phase(
            package_dir,
            subpath,
            resolution_mode,
            type_resolver,
        ),
    )
}

pub(crate) fn resolve_vue3_package_json_type_reference_entry_phase(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonPhaseResolution {
    let enable_exports = type_resolver
        .package_json_features_for_type_reference(resolution_mode.is_some())
        .exports;
    let exports_mode = vue3_type_reference_package_resolution_mode(resolution_mode, type_resolver);
    resolve_vue3_package_json_entry_phase_with_exports(
        package_dir,
        subpath,
        type_resolver,
        exports_mode,
        true,
        enable_exports,
        true,
        Vue3PackageResolutionPhase::Types,
    )
}

pub(crate) fn resolve_vue3_package_json_type_reference_directory_entry_phase(
    package_dir: &Path,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonPhaseResolution {
    resolve_vue3_package_json_entry_phase_with_exports(
        package_dir,
        None,
        type_resolver,
        vue3_type_reference_package_resolution_mode(resolution_mode, type_resolver),
        true,
        false,
        false,
        Vue3PackageResolutionPhase::Types,
    )
}

fn vue3_type_reference_package_resolution_mode(
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3TypeResolutionMode> {
    resolution_mode.or(match type_resolver.module_resolution {
        Vue3TypeModuleResolutionKind::Bundler => Some(Vue3TypeResolutionMode::Import),
        Vue3TypeModuleResolutionKind::Node16 | Vue3TypeModuleResolutionKind::NodeNext => {
            Some(Vue3TypeResolutionMode::Require)
        }
        Vue3TypeModuleResolutionKind::Classic | Vue3TypeModuleResolutionKind::Node10 => None,
    })
}

#[cfg(test)]
fn resolve_vue3_package_json_type_entry_with_exports(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
    exports_mode: Option<Vue3TypeResolutionMode>,
    declaration_only: bool,
    enable_exports: bool,
    apply_bare_package_rules: bool,
) -> Vue3PackageJsonTypeResolution {
    let type_phase = resolve_vue3_package_json_entry_phase_with_exports(
        package_dir,
        subpath,
        type_resolver,
        exports_mode,
        declaration_only,
        enable_exports,
        apply_bare_package_rules,
        Vue3PackageResolutionPhase::Types,
    );
    match type_phase {
        Vue3PackageJsonPhaseResolution::Resolved(path) => {
            return Vue3PackageJsonTypeResolution::Resolved(path);
        }
        Vue3PackageJsonPhaseResolution::Blocked => {
            return Vue3PackageJsonTypeResolution::Blocked;
        }
        Vue3PackageJsonPhaseResolution::NoPackageJson => {
            return Vue3PackageJsonTypeResolution::NoPackageJson;
        }
        Vue3PackageJsonPhaseResolution::Missing(fallback) if declaration_only => {
            return vue3_package_missing_phase_resolution(fallback);
        }
        Vue3PackageJsonPhaseResolution::Missing(_) => {}
    }
    vue3_package_phase_resolution_to_type_resolution(
        resolve_vue3_package_json_entry_phase_with_exports(
            package_dir,
            subpath,
            type_resolver,
            exports_mode,
            false,
            enable_exports,
            apply_bare_package_rules,
            Vue3PackageResolutionPhase::JavaScript,
        ),
    )
}

pub(crate) fn resolve_vue3_package_json_entry_phase_with_mode(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonPhaseResolution {
    resolve_vue3_package_json_entry_phase_with_exports(
        package_dir,
        subpath,
        type_resolver,
        Some(resolution_mode),
        false,
        type_resolver.package_json_features().exports,
        true,
        phase,
    )
}

pub(crate) fn resolve_vue3_package_json_directory_entry_phase_with_mode(
    package_dir: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonPhaseResolution {
    resolve_vue3_package_json_entry_phase_with_exports(
        package_dir,
        None,
        type_resolver,
        Some(resolution_mode),
        false,
        false,
        false,
        phase,
    )
}

fn resolve_vue3_package_json_entry_phase_with_exports(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
    exports_mode: Option<Vue3TypeResolutionMode>,
    declaration_only: bool,
    enable_exports: bool,
    apply_bare_package_rules: bool,
    phase: Vue3PackageResolutionPhase,
) -> Vue3PackageJsonPhaseResolution {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageJsonPhaseResolution::Blocked;
    }
    let Some(_resolution_guard) = type_resolver
        .external_type_session
        .begin_package_resolution(package_dir)
    else {
        return Vue3PackageJsonPhaseResolution::Blocked;
    };
    let package_json = package_dir.join("package.json");
    let Some(manifest) = type_resolver
        .external_type_session
        .package_json_from_path(&package_json)
    else {
        return if type_resolver.external_type_session.metadata_is_blocked() {
            Vue3PackageJsonPhaseResolution::Blocked
        } else {
            Vue3PackageJsonPhaseResolution::NoPackageJson
        };
    };
    if enable_exports {
        if let Some((exports, resolution_mode)) = manifest
            .exports
            .as_ref()
            .zip(exports_mode)
            .filter(|(exports, _)| vue3_package_json_value_is_truthy(exports))
        {
            return match resolve_vue3_package_exports_type_path(
                package_dir,
                exports,
                subpath,
                resolution_mode,
                phase,
                declaration_only,
                type_resolver,
            ) {
                Vue3PackageJsonTypeResolution::Resolved(path) => {
                    Vue3PackageJsonPhaseResolution::Resolved(path)
                }
                _ => Vue3PackageJsonPhaseResolution::Blocked,
            };
        }
    }
    let root_target = if subpath.is_none() {
        match phase {
            Vue3PackageResolutionPhase::Types => {
                vue3_package_json_path_field(manifest.typings.as_ref())
                    .or_else(|| vue3_package_json_path_field(manifest.types.as_ref()))
                    .or_else(|| vue3_package_json_path_field(manifest.main.as_ref()))
            }
            Vue3PackageResolutionPhase::JavaScript => {
                vue3_package_json_path_field(manifest.main.as_ref())
            }
        }
    } else {
        None
    };
    let root_target = match root_target {
        Some(target) => {
            if !type_resolver
                .external_type_session
                .claim_metadata_target_steps(target.len())
            {
                return Vue3PackageJsonPhaseResolution::Blocked;
            }
            let Some(target) =
                vue3_normalize_typescript_path_separators(target, type_resolver)
            else {
                return Vue3PackageJsonPhaseResolution::Blocked;
            };
            if !vue3_package_type_target_is_safe(&target) {
                return Vue3PackageJsonPhaseResolution::Blocked;
            }
            Some(target)
        }
        None => None,
    };
    let path_resolution_mode = exports_mode.unwrap_or(Vue3TypeResolutionMode::Import);
    let types_versions_target_policy = vue3_package_types_versions_target_policy(
        subpath,
        manifest.module_type,
        manifest.exports.as_ref(),
        path_resolution_mode,
        type_resolver,
    );
    let types_versions_source = subpath
        .map(|subpath| subpath.trim_start_matches("./"))
        .or_else(|| root_target.as_deref().map(|target| target.trim_start_matches("./")))
        .unwrap_or("index");
    let types_versions_resolution = vue3_package_types_versions_path(
        package_dir,
        &manifest.types_versions,
        types_versions_source,
        path_resolution_mode,
        types_versions_target_policy,
        phase,
        declaration_only,
        type_resolver,
    );
    match types_versions_resolution {
        Vue3TypesVersionsResolution::Resolved(path) => {
            return Vue3PackageJsonPhaseResolution::Resolved(path);
        }
        Vue3TypesVersionsResolution::Blocked => {
            return Vue3PackageJsonPhaseResolution::Blocked;
        }
        Vue3TypesVersionsResolution::MatchedButMissing => {
            return vue3_package_missing_phase(
                subpath,
                exports_mode,
                enable_exports,
                apply_bare_package_rules,
                manifest.exports.as_ref(),
                false,
                type_resolver,
            );
        }
        Vue3TypesVersionsResolution::NotMatched => {}
    }
    if subpath.is_none() {
        if let Some(target) = root_target.as_deref() {
            let policy = vue3_package_root_field_target_policy(
                manifest.module_type,
                path_resolution_mode,
                type_resolver,
            );
            let resolved = match phase {
                Vue3PackageResolutionPhase::Types => vue3_package_type_field_path_with_mode(
                    package_dir,
                    target,
                    path_resolution_mode,
                    policy,
                    type_resolver,
                ),
                Vue3PackageResolutionPhase::JavaScript => {
                    vue3_package_main_javascript_path(
                        package_dir,
                        target,
                        policy,
                        type_resolver,
                    )
                }
            };
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vue3PackageJsonPhaseResolution::Blocked;
            }
            if let Some(resolved) = resolved {
                return Vue3PackageJsonPhaseResolution::Resolved(resolved);
            }
        }
    }
    if let Some(subpath) = subpath.filter(|_| {
        types_versions_target_policy
            == Vue3PackageTypeTargetPolicy::RequireExplicitFileNameWithLegacyIndexJsFallback
    }) {
        let Some(candidate) =
            vue3_package_type_target_candidate(package_dir, subpath, type_resolver)
        else {
            return if type_resolver.external_type_session.metadata_is_blocked() {
                Vue3PackageJsonPhaseResolution::Blocked
            } else {
                vue3_package_missing_phase(
                    Some(subpath),
                    exports_mode,
                    enable_exports,
                    apply_bare_package_rules,
                    manifest.exports.as_ref(),
                    false,
                    type_resolver,
                )
            };
        };
        let resolved = resolve_vue3_package_target_candidate_for_phase_with_policy(
            &candidate,
            path_resolution_mode,
            types_versions_target_policy,
            phase,
            false,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vue3PackageJsonPhaseResolution::Blocked;
        }
        if let Some(resolved) = resolved {
            return Vue3PackageJsonPhaseResolution::Resolved(resolved);
        }
        return vue3_package_missing_phase(
            Some(subpath),
            exports_mode,
            enable_exports,
            apply_bare_package_rules,
            manifest.exports.as_ref(),
            false,
            type_resolver,
        );
    }
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageJsonPhaseResolution::Blocked;
    }
    vue3_package_missing_phase(
        subpath,
        exports_mode,
        enable_exports,
        apply_bare_package_rules,
        manifest.exports.as_ref(),
        true,
        type_resolver,
    )
}

fn vue3_package_missing_phase(
    subpath: Option<&str>,
    exports_mode: Option<Vue3TypeResolutionMode>,
    enable_exports: bool,
    apply_bare_package_rules: bool,
    exports: Option<&serde_json::Value>,
    allowed: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonPhaseResolution {
    let nested_manifest_is_suppressed = apply_bare_package_rules
        && subpath.is_some()
        && enable_exports
        && exports.is_some();
    let node_esm_root_index_is_suppressed = apply_bare_package_rules
        && subpath.is_none()
        && exports_mode.is_some_and(|resolution_mode| {
            type_resolver
                .module_resolution
                .uses_node_esm_specifier_rules(
                    resolution_mode,
                    &type_resolver.typescript_version,
                )
        })
        && exports.is_some_and(|exports| !exports.is_null());
    Vue3PackageJsonPhaseResolution::Missing(Vue3PackagePathFallback {
        allowed,
        allow_nested_manifest: !nested_manifest_is_suppressed,
        allow_index: !node_esm_root_index_is_suppressed,
    })
}

#[cfg(test)]
fn vue3_package_phase_resolution_to_type_resolution(
    resolution: Vue3PackageJsonPhaseResolution,
) -> Vue3PackageJsonTypeResolution {
    match resolution {
        Vue3PackageJsonPhaseResolution::NoPackageJson => {
            Vue3PackageJsonTypeResolution::NoPackageJson
        }
        Vue3PackageJsonPhaseResolution::Missing(fallback) => {
            vue3_package_missing_phase_resolution(fallback)
        }
        Vue3PackageJsonPhaseResolution::Resolved(path) => {
            Vue3PackageJsonTypeResolution::Resolved(path)
        }
        Vue3PackageJsonPhaseResolution::Blocked => Vue3PackageJsonTypeResolution::Blocked,
    }
}

#[cfg(test)]
fn vue3_package_missing_phase_resolution(
    fallback: Vue3PackagePathFallback,
) -> Vue3PackageJsonTypeResolution {
    if !fallback.allow_index {
        Vue3PackageJsonTypeResolution::NoPackageTypeEntryWithoutIndex
    } else if !fallback.allow_nested_manifest {
        Vue3PackageJsonTypeResolution::NoPackageTypeEntryWithoutNestedManifest
    } else {
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry
    }
}

#[cfg(test)]
pub(crate) fn vue3_package_exports_type_target(
    exports: &serde_json::Value,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    vue3_package_exports_type_target_with_mode(
        exports,
        subpath,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

#[cfg(test)]
pub(crate) fn vue3_package_exports_type_target_with_mode(
    exports: &serde_json::Value,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<String> {
    let mut selected = None;
    let result = visit_vue3_package_exports_type_targets(
        exports,
        subpath,
        resolution_mode,
        type_resolver,
        &mut |target| {
            selected = Some(target.to_string());
            Vue3PackageTargetVisit::Resolved
        },
    );
    (result == Vue3PackageTargetVisit::Resolved)
        .then_some(selected)
        .flatten()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Vue3PackageTargetVisit {
    Resolved,
    Missing,
    NullTarget,
    Invalid,
    Blocked,
}

#[derive(Clone, Copy)]
enum Vue3PackageTargetKind {
    Exports,
    Imports,
}

#[derive(Clone, Copy)]
enum Vue3PackageTargetExpansion<'a> {
    Exact,
    Pattern(&'a str),
    Prefix(&'a str),
}

fn resolve_vue3_package_exports_type_path(
    package_dir: &Path,
    exports: &serde_json::Value,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    declaration_only: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    let mut resolved = None;
    let result = visit_vue3_package_exports_type_targets(
        exports,
        subpath,
        resolution_mode,
        type_resolver,
        &mut |target| {
            let candidate = if declaration_only {
                vue3_package_export_type_reference_path(package_dir, target, type_resolver)
            } else {
                vue3_package_export_path_for_phase_with_mode(
                    package_dir,
                    target,
                    resolution_mode,
                    phase,
                    type_resolver,
                )
            };
            if type_resolver.external_type_session.metadata_is_blocked() {
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
            Vue3PackageJsonTypeResolution::Resolved(path)
        }
        _ => Vue3PackageJsonTypeResolution::Blocked,
    }
}

fn visit_vue3_package_exports_type_targets(
    exports: &serde_json::Value,
    subpath: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    visitor: &mut impl FnMut(&str) -> Vue3PackageTargetVisit,
) -> Vue3PackageTargetVisit {
    let key = subpath
        .map(|subpath| format!("./{}", subpath.trim_start_matches("./")))
        .unwrap_or_else(|| ".".into());
    let Some(object) = exports.as_object() else {
        return if key == "." {
            visit_vue3_package_target(
                exports,
                resolution_mode,
                Vue3PackageTargetExpansion::Exact,
                Vue3PackageTargetKind::Exports,
                type_resolver,
                visitor,
            )
        } else {
            Vue3PackageTargetVisit::Missing
        };
    };
    if key == "." {
        let mut conditions_only = true;
        for object_key in object.keys() {
            if !type_resolver
                .external_type_session
                .claim_metadata_match_steps(object_key.len())
            {
                return Vue3PackageTargetVisit::Blocked;
            }
            if object_key.starts_with('.') {
                conditions_only = false;
                break;
            }
        }
        return if conditions_only {
            visit_vue3_package_target(
                exports,
                resolution_mode,
                Vue3PackageTargetExpansion::Exact,
                Vue3PackageTargetKind::Exports,
                type_resolver,
                visitor,
            )
        } else if let Some(target) = object.get(".") {
            visit_vue3_package_target(
                target,
                resolution_mode,
                Vue3PackageTargetExpansion::Exact,
                Vue3PackageTargetKind::Exports,
                type_resolver,
                visitor,
            )
        } else {
            Vue3PackageTargetVisit::Missing
        };
    }
    for object_key in object.keys() {
        if !type_resolver
            .external_type_session
            .claim_metadata_match_steps(object_key.len())
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if !object_key.starts_with('.') {
            return Vue3PackageTargetVisit::Missing;
        }
    }
    if !type_resolver
        .external_type_session
        .claim_metadata_match_steps(key.len())
    {
        return Vue3PackageTargetVisit::Blocked;
    }
    if let Some(target) = object.get(&key) {
        return visit_vue3_package_target(
            target,
            resolution_mode,
            Vue3PackageTargetExpansion::Exact,
            Vue3PackageTargetKind::Exports,
            type_resolver,
            visitor,
        );
    }
    let mut selected = None;
    for (pattern, target) in object {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if !type_resolver
            .external_type_session
            .claim_metadata_match_steps(pattern.len().saturating_add(key.len()))
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        let expansion = if pattern.contains('*') {
            let Some(capture) = vue3_package_export_pattern_capture(pattern, &key) else {
                continue;
            };
            Vue3PackageTargetExpansion::Pattern(capture)
        } else if pattern.ends_with('/') && key.starts_with(pattern) {
            Vue3PackageTargetExpansion::Prefix(&key[pattern.len()..])
        } else {
            continue;
        };
        let specificity = vue3_package_expansion_specificity(pattern);
        if selected
            .as_ref()
            .is_none_or(|(current, _, _)| specificity > *current)
        {
            selected = Some((specificity, target, expansion));
        }
    }
    let Some((_, target, expansion)) = selected else {
        return Vue3PackageTargetVisit::Missing;
    };
    visit_vue3_package_target(
        target,
        resolution_mode,
        expansion,
        Vue3PackageTargetKind::Exports,
        type_resolver,
        visitor,
    )
}

fn visit_vue3_package_imports_type_targets(
    imports: &serde_json::Value,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    visitor: &mut impl FnMut(&str) -> Vue3PackageTargetVisit,
) -> Vue3PackageTargetVisit {
    if source.contains('*')
        || !vue3_package_import_specifier_is_safe_for_resolver(source, type_resolver)
    {
        return Vue3PackageTargetVisit::Invalid;
    }
    let Some(object) = imports.as_object() else {
        return Vue3PackageTargetVisit::Invalid;
    };
    if !type_resolver
        .external_type_session
        .claim_metadata_match_steps(source.len())
    {
        return Vue3PackageTargetVisit::Blocked;
    }
    if let Some(target) = object.get(source) {
        return visit_vue3_package_target(
            target,
            resolution_mode,
            Vue3PackageTargetExpansion::Exact,
            Vue3PackageTargetKind::Imports,
            type_resolver,
            visitor,
        );
    }

    let mut selected = None;
    for (pattern, target) in object {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if !type_resolver
            .external_type_session
            .claim_metadata_match_steps(pattern.len().saturating_add(source.len()))
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if !vue3_package_import_expansion_key_is_safe(pattern, type_resolver) {
            continue;
        }
        let expansion = if pattern.contains('*') {
            let Some(capture) = vue3_package_export_pattern_capture(pattern, source) else {
                continue;
            };
            Vue3PackageTargetExpansion::Pattern(capture)
        } else if pattern.ends_with('/') && source.starts_with(pattern) {
            Vue3PackageTargetExpansion::Prefix(&source[pattern.len()..])
        } else {
            continue;
        };
        let specificity = vue3_package_expansion_specificity(pattern);
        if selected
            .as_ref()
            .is_none_or(|(current, _, _)| specificity > *current)
        {
            selected = Some((specificity, target, expansion));
        }
    }
    let Some((_, target, expansion)) = selected else {
        return Vue3PackageTargetVisit::Missing;
    };
    visit_vue3_package_target(
        target,
        resolution_mode,
        expansion,
        Vue3PackageTargetKind::Imports,
        type_resolver,
        visitor,
    )
}

fn visit_vue3_package_target(
    target: &serde_json::Value,
    resolution_mode: Vue3TypeResolutionMode,
    expansion: Vue3PackageTargetExpansion<'_>,
    target_kind: Vue3PackageTargetKind,
    type_resolver: &Vue3TypeResolverContext,
    visitor: &mut impl FnMut(&str) -> Vue3PackageTargetVisit,
) -> Vue3PackageTargetVisit {
    if let Some(target) = target.as_str() {
        if !type_resolver
            .external_type_session
            .claim_metadata_target_steps(target.len())
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        let target = if target.starts_with("./") && target.contains('\\') {
            let Some(target) =
                vue3_normalize_typescript_path_separators(target, type_resolver)
            else {
                return Vue3PackageTargetVisit::Blocked;
            };
            std::borrow::Cow::Owned(target)
        } else {
            std::borrow::Cow::Borrowed(target)
        };
        let target = target.as_ref();
        let prefix_expansion = matches!(expansion, Vue3PackageTargetExpansion::Prefix(_));
        if !vue3_package_target_is_safe(
            target,
            target_kind,
            prefix_expansion,
            type_resolver,
        ) {
            return Vue3PackageTargetVisit::Invalid;
        }
        let expansion_steps = match expansion {
            Vue3PackageTargetExpansion::Pattern(capture) => capture.len().saturating_mul(
                target
                    .as_bytes()
                    .iter()
                    .filter(|byte| **byte == b'*')
                    .count(),
            ),
            Vue3PackageTargetExpansion::Prefix(subpath) if target.ends_with('/') => subpath.len(),
            Vue3PackageTargetExpansion::Exact
            | Vue3PackageTargetExpansion::Prefix(_) => 0,
        };
        if expansion_steps != 0
            && !type_resolver
                .external_type_session
                .claim_metadata_target_steps(expansion_steps)
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        let expanded = match expansion {
            Vue3PackageTargetExpansion::Exact => target.to_string(),
            Vue3PackageTargetExpansion::Pattern(capture) => {
                if target.contains('*') {
                    if !vue3_package_export_pattern_capture_is_safe(capture)
                        || !type_resolver
                            .external_type_session
                            .metadata_path_is_within_limit(capture)
                    {
                        return Vue3PackageTargetVisit::Invalid;
                    }
                    let Some(expanded) = type_resolver
                        .external_type_session
                        .replace_metadata_path_pattern(target, "*", capture)
                    else {
                        return Vue3PackageTargetVisit::Blocked;
                    };
                    expanded
                } else {
                    target.to_string()
                }
            }
            Vue3PackageTargetExpansion::Prefix(subpath) => {
                if !target.ends_with('/')
                    || !vue3_package_export_pattern_capture_is_safe(subpath)
                    || !type_resolver
                        .external_type_session
                        .metadata_path_is_within_limit(subpath)
                {
                    return Vue3PackageTargetVisit::Invalid;
                }
                let Some(expanded) = type_resolver
                    .external_type_session
                    .concat_metadata_path(target, subpath)
                else {
                    return Vue3PackageTargetVisit::Blocked;
                };
                expanded
            }
        };
        if vue3_package_export_contains_encoded_separator(&expanded)
            || !vue3_package_target_is_safe(&expanded, target_kind, false, type_resolver)
        {
            return Vue3PackageTargetVisit::Invalid;
        }
        if !type_resolver
            .external_type_session
            .metadata_path_is_within_limit(&expanded)
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        return visitor(&expanded);
    }
    if target.is_null() {
        return Vue3PackageTargetVisit::NullTarget;
    }
    if let Some(targets) = target.as_array() {
        let mut fallback = if targets.is_empty() {
            Vue3PackageTargetVisit::Invalid
        } else {
            Vue3PackageTargetVisit::Missing
        };
        for target in targets {
            if !type_resolver
                .external_type_session
                .claim_metadata_fanout_entry()
            {
                return Vue3PackageTargetVisit::Blocked;
            }
            match visit_vue3_package_target(
                target,
                resolution_mode,
                expansion,
                target_kind,
                type_resolver,
                visitor,
            ) {
                Vue3PackageTargetVisit::Missing => {}
                Vue3PackageTargetVisit::NullTarget
                    if vue3_package_null_target_stops_fallback(type_resolver) =>
                {
                    return Vue3PackageTargetVisit::NullTarget;
                }
                result @ (Vue3PackageTargetVisit::NullTarget
                | Vue3PackageTargetVisit::Invalid) => fallback = result,
                result => return result,
            }
        }
        return fallback;
    }
    let Some(conditions) = target.as_object() else {
        return Vue3PackageTargetVisit::Invalid;
    };
    for condition in conditions.keys() {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if !type_resolver
            .external_type_session
            .claim_metadata_match_steps(condition.len())
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if !vue3_package_export_builtin_condition_is_active(
            condition,
            resolution_mode,
            type_resolver,
        ) {
            let custom_condition_steps = type_resolver
                .custom_conditions
                .lookup_match_steps(condition);
            if custom_condition_steps > 0
                && !type_resolver
                    .external_type_session
                    .claim_metadata_match_steps(custom_condition_steps)
            {
                return Vue3PackageTargetVisit::Blocked;
            }
        }
    }
    for (condition, target) in conditions {
        if !vue3_package_export_condition_is_active(
            condition,
            resolution_mode,
            type_resolver,
        ) {
            continue;
        }
        match visit_vue3_package_target(
            target,
            resolution_mode,
            expansion,
            target_kind,
            type_resolver,
            visitor,
        ) {
            Vue3PackageTargetVisit::Missing | Vue3PackageTargetVisit::Invalid => {}
            Vue3PackageTargetVisit::NullTarget
                if !vue3_package_null_target_stops_fallback(type_resolver) => {}
            result => return result,
        }
    }
    Vue3PackageTargetVisit::Missing
}

fn vue3_package_null_target_stops_fallback(
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    // TypeScript 6 made null a terminal empty SearchResult; older releases try the next target.
    type_resolver.typescript_version >= (6, 0, 0).into()
}

fn vue3_package_json_value_is_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_none_or(|value| value != 0.0),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

pub(crate) fn vue3_package_json_has_truthy_exports(
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return false;
    }
    type_resolver
        .external_type_session
        .package_json_from_path(&package_dir.join("package.json"))
        .is_some_and(|manifest| {
            manifest
                .exports
                .as_ref()
                .is_some_and(vue3_package_json_value_is_truthy)
        })
}

fn vue3_package_json_path_field(value: Option<&serde_json::Value>) -> Option<&str> {
    value
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

fn vue3_package_target_is_safe(
    target: &str,
    target_kind: Vue3PackageTargetKind,
    allow_trailing_slash: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    match target_kind {
        Vue3PackageTargetKind::Exports => vue3_package_export_target_is_safe(target),
        Vue3PackageTargetKind::Imports => {
            if target.starts_with("./") {
                vue3_package_export_target_is_safe(target)
            } else if target.starts_with('#') {
                vue3_package_import_specifier_is_safe_with_trailing_slash(
                    target,
                    allow_trailing_slash,
                    type_resolver.package_json_features().imports_pattern_root,
                )
            } else {
                vue3_package_import_external_target_is_safe_with_trailing_slash(
                    target,
                    allow_trailing_slash,
                )
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn vue3_package_import_external_target_is_safe(target: &str) -> bool {
    vue3_package_import_external_target_is_safe_with_trailing_slash(target, false)
}

fn vue3_package_import_external_target_is_safe_with_trailing_slash(
    target: &str,
    allow_trailing_slash: bool,
) -> bool {
    let target = if allow_trailing_slash {
        target.strip_suffix('/').unwrap_or(target)
    } else {
        target
    };
    vue3_package_import_parts(target).is_some()
        && !vue3_package_export_contains_encoded_separator(target)
        && target.split('/').all(|segment| {
            !segment.is_empty() && !vue3_package_export_segment_is_forbidden(segment)
        })
}

#[cfg(test)]
pub(crate) fn vue3_package_import_specifier_is_safe(source: &str) -> bool {
    vue3_package_import_specifier_is_safe_with_trailing_slash(source, false, false)
}

fn vue3_package_import_specifier_is_safe_for_resolver(
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    vue3_package_import_specifier_is_safe_with_trailing_slash(
        source,
        false,
        type_resolver.package_json_features().imports_pattern_root,
    )
}

fn vue3_package_import_specifier_is_safe_with_trailing_slash(
    source: &str,
    allow_trailing_slash: bool,
    allow_pattern_root: bool,
) -> bool {
    let source = if allow_trailing_slash {
        source.strip_suffix('/').unwrap_or(source)
    } else {
        source
    };
    if source == "#"
        || !source.starts_with('#')
        || (source.starts_with("#/") && !allow_pattern_root)
        || source.ends_with('/')
        || source.contains('\\')
        || vue3_package_export_contains_encoded_separator(source)
    {
        return false;
    }
    let body = if allow_pattern_root {
        source.strip_prefix("#/").unwrap_or(&source[1..])
    } else {
        &source[1..]
    };
    !body.is_empty()
        && body
            .split('/')
            .all(|segment| !segment.is_empty() && !vue3_package_export_segment_is_forbidden(segment))
}

fn vue3_package_import_expansion_key_is_safe(
    key: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    key.bytes().filter(|byte| *byte == b'*').count() <= 1
        && vue3_package_import_specifier_is_safe_with_trailing_slash(
            key,
            true,
            type_resolver.package_json_features().imports_pattern_root,
        )
}

fn vue3_package_expansion_specificity(key: &str) -> (usize, bool, usize) {
    let star = key.find('*');
    (
        star.map_or(key.len(), |star| star + 1),
        star.is_some(),
        key.len(),
    )
}

fn vue3_package_export_condition_is_active(
    condition: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    vue3_package_export_builtin_condition_is_active(
        condition,
        resolution_mode,
        type_resolver,
    ) || type_resolver.custom_conditions.contains(condition)
        || condition.strip_prefix("types@").is_some_and(|selector| {
            vue3_package_types_version_selector_matches_version(
                selector,
                &type_resolver.typescript_version,
            )
        })
}

fn vue3_package_export_builtin_condition_is_active(
    condition: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    condition == "types"
        || (condition == "node"
            && type_resolver.module_resolution != Vue3TypeModuleResolutionKind::Bundler)
        || condition == "default"
        || matches!(
            (condition, resolution_mode),
            ("import", Vue3TypeResolutionMode::Import)
                | ("require", Vue3TypeResolutionMode::Require)
        )
}

pub(crate) fn vue3_package_export_pattern_capture<'a>(
    pattern: &str,
    key: &'a str,
) -> Option<&'a str> {
    let star = pattern.find('*')?;
    if pattern[star + 1..].contains('*') {
        return None;
    }
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !key.starts_with(prefix) || !key.ends_with(suffix) || key.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some(&key[prefix.len()..key.len() - suffix.len()])
}

pub(crate) fn vue3_package_export_target_is_safe(target: &str) -> bool {
    let Some(relative) = target.strip_prefix("./") else {
        return false;
    };
    vue3_package_type_target_is_safe(target)
        && !vue3_package_export_contains_encoded_separator(target)
        && vue3_package_export_segments_are_safe(relative)
}

pub(crate) fn vue3_package_export_pattern_capture_is_safe(capture: &str) -> bool {
    vue3_package_export_segments_are_safe(capture)
}

fn vue3_package_export_segments_are_safe(value: &str) -> bool {
    value
        .split(['/', '\\'])
        .all(|segment| !vue3_package_export_segment_is_forbidden(segment))
}

fn vue3_package_export_segment_is_forbidden(segment: &str) -> bool {
    [b".".as_slice(), b"..".as_slice(), b"node_modules".as_slice()]
        .into_iter()
        .any(|forbidden| vue3_package_export_percent_decoded_eq(segment, forbidden))
}

fn vue3_package_export_percent_decoded_eq(value: &str, expected: &[u8]) -> bool {
    let bytes = value.as_bytes();
    let mut value_index = 0;
    let mut expected_index = 0;
    while value_index < bytes.len() && expected_index < expected.len() {
        let percent_decoded = if bytes[value_index] == b'%' && value_index + 2 < bytes.len() {
            vue3_package_export_hex_value(bytes[value_index + 1])
                .zip(vue3_package_export_hex_value(bytes[value_index + 2]))
                .map(|(high, low)| (high << 4) | low)
        } else {
            None
        };
        let decoded = if let Some(decoded) = percent_decoded {
            value_index += 3;
            decoded
        } else {
            let byte = bytes[value_index];
            value_index += 1;
            byte
        };
        if decoded.to_ascii_lowercase() != expected[expected_index] {
            return false;
        }
        expected_index += 1;
    }
    value_index == bytes.len() && expected_index == expected.len()
}

fn vue3_package_export_contains_encoded_separator(value: &str) -> bool {
    value.as_bytes().windows(3).any(|window| {
        window[0] == b'%'
            && matches!(
                (
                    vue3_package_export_hex_value(window[1]),
                    vue3_package_export_hex_value(window[2]),
                ),
                (Some(2), Some(15)) | (Some(5), Some(12))
            )
    })
}

fn vue3_package_export_hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn vue3_package_export_path_for_phase_with_mode(
    package_dir: &Path,
    target: &str,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !target.starts_with("./") {
        return None;
    }
    let candidate = vue3_package_type_target_candidate(package_dir, target, type_resolver)?;
    match phase {
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
    }
}

fn vue3_package_export_type_reference_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !target.starts_with("./") || !vue3_package_type_target_is_safe(target) {
        return None;
    }
    let candidate = normalize_path_components(package_dir.join(target));
    resolve_vue3_metadata_package_target_declaration_file(&candidate, type_resolver)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3TypesVersionsResolution {
    NotMatched,
    MatchedButMissing,
    Resolved(PathBuf),
    Blocked,
}

pub(crate) fn resolve_vue3_package_subpath_index_types_versions_phase(
    package_dir: &Path,
    subpath_dir: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    phase: Vue3PackageResolutionPhase,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3TypesVersionsResolution {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3TypesVersionsResolution::Blocked;
    }
    let Some(_resolution_guard) = type_resolver
        .external_type_session
        .begin_package_resolution(package_dir)
    else {
        return Vue3TypesVersionsResolution::Blocked;
    };
    let Some(manifest) = type_resolver
        .external_type_session
        .package_json_from_path(&package_dir.join("package.json"))
    else {
        return if type_resolver.external_type_session.metadata_is_blocked() {
            Vue3TypesVersionsResolution::Blocked
        } else {
            Vue3TypesVersionsResolution::NotMatched
        };
    };
    let target_policy = vue3_package_types_versions_target_policy(
        Some(""),
        manifest.module_type,
        manifest.exports.as_ref(),
        resolution_mode,
        type_resolver,
    );
    vue3_package_types_versions_path(
        subpath_dir,
        &manifest.types_versions,
        "index",
        resolution_mode,
        target_policy,
        phase,
        false,
        type_resolver,
    )
}

fn vue3_package_types_versions_path(
    package_dir: &Path,
    types_versions: &Vue3PackageTypesVersions,
    source: &str,
    resolution_mode: Vue3TypeResolutionMode,
    target_policy: Vue3PackageTypeTargetPolicy,
    phase: Vue3PackageResolutionPhase,
    declaration_only: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3TypesVersionsResolution {
    let Some(mappings) = vue3_package_types_versions_mapping(types_versions, type_resolver) else {
        return if type_resolver.external_type_session.metadata_is_blocked() {
            Vue3TypesVersionsResolution::Blocked
        } else {
            Vue3TypesVersionsResolution::NotMatched
        };
    };
    let Some((mapping_index, capture)) = vue3_typescript_best_path_pattern_match(
        mappings
            .0
            .iter()
            .enumerate()
            .map(|(index, (pattern, _))| (index, pattern.as_str())),
        source,
        type_resolver,
    ) else {
        return if type_resolver.external_type_session.metadata_is_blocked() {
            Vue3TypesVersionsResolution::Blocked
        } else {
            Vue3TypesVersionsResolution::NotMatched
        };
    };
    let targets = &mappings.0[mapping_index].1;
    for target in vue3_tsconfig_path_target_values(targets) {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return Vue3TypesVersionsResolution::Blocked;
        }
        if !type_resolver
            .external_type_session
            .claim_metadata_target_steps(target.len())
        {
            return Vue3TypesVersionsResolution::Blocked;
        }
        if !capture.is_empty()
            && target.contains('*')
            && !type_resolver
                .external_type_session
                .claim_metadata_target_steps(capture.len())
        {
            return Vue3TypesVersionsResolution::Blocked;
        }
        let try_raw_target = vue3_types_versions_target_has_known_extension(target)
            && (!declaration_only
                || vue3_types_versions_target_has_typescript_extension(target));
        let Some(target) =
            vue3_typescript_path_target_substitution(target, &capture, type_resolver)
        else {
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vue3TypesVersionsResolution::Blocked;
            }
            continue;
        };
        let Some(target) = vue3_normalize_typescript_path_separators(&target, type_resolver)
        else {
            return Vue3TypesVersionsResolution::Blocked;
        };
        if !vue3_package_type_target_is_safe(&target) {
            type_resolver.external_type_session.block_metadata();
            return Vue3TypesVersionsResolution::Blocked;
        }
        let Some(candidate) =
            vue3_package_type_target_candidate(package_dir, &target, type_resolver)
        else {
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vue3TypesVersionsResolution::Blocked;
            }
            continue;
        };
        let resolved = resolve_vue3_package_target_candidate_for_phase_with_policy(
            &candidate,
            resolution_mode,
            target_policy,
            phase,
            try_raw_target,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return Vue3TypesVersionsResolution::Blocked;
        }
        if let Some(resolved) = resolved {
            return Vue3TypesVersionsResolution::Resolved(resolved);
        }
    }
    Vue3TypesVersionsResolution::MatchedButMissing
}

fn vue3_types_versions_target_has_known_extension(target: &str) -> bool {
    const EXTENSIONS: [&str; 12] = [
        ".d.ts", ".d.mts", ".d.cts", ".mjs", ".mts", ".cjs", ".cts", ".ts", ".js",
        ".tsx", ".jsx", ".json",
    ];
    EXTENSIONS
        .into_iter()
        .any(|extension| target.ends_with(extension))
}

fn vue3_types_versions_target_has_typescript_extension(target: &str) -> bool {
    [".ts", ".tsx", ".mts", ".cts"]
        .into_iter()
        .any(|extension| target.ends_with(extension))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Vue3PackageTypeTargetPolicy {
    AllowImplicit,
    RequireExplicitFileName,
    RequireExplicitFileNameWithLegacyIndexJsFallback,
}

impl Vue3PackageTypeTargetPolicy {
    fn path_policy(self) -> Vue3PackageTargetPathPolicy {
        match self {
            Self::AllowImplicit => Vue3PackageTargetPathPolicy::AllowImplicit,
            Self::RequireExplicitFileName
            | Self::RequireExplicitFileNameWithLegacyIndexJsFallback => {
                Vue3PackageTargetPathPolicy::RequireExplicitFileName
            }
        }
    }
}

fn resolve_vue3_package_target_candidate_for_phase_with_policy(
    candidate: &Path,
    resolution_mode: Vue3TypeResolutionMode,
    policy: Vue3PackageTypeTargetPolicy,
    phase: Vue3PackageResolutionPhase,
    try_raw_target: bool,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let resolved = match phase {
        Vue3PackageResolutionPhase::Types => {
            resolve_vue3_metadata_types_versions_type_target_path_with_mode(
                candidate,
                resolution_mode,
                policy.path_policy(),
                try_raw_target,
                type_resolver,
            )
        }
        Vue3PackageResolutionPhase::JavaScript => {
            resolve_vue3_metadata_types_versions_javascript_target_path(
                candidate,
                policy.path_policy(),
                try_raw_target,
                type_resolver,
            )
        }
    };
    if resolved.is_some()
        || type_resolver.external_type_session.metadata_is_blocked()
        || policy
            != Vue3PackageTypeTargetPolicy::RequireExplicitFileNameWithLegacyIndexJsFallback
    {
        return resolved;
    }
    match phase {
        Vue3PackageResolutionPhase::Types => {
            resolve_vue3_metadata_legacy_package_type_field_path_with_mode(
                &candidate.join("index.js"),
                resolution_mode,
                Vue3PackageTargetPathPolicy::RequireExplicitFileName,
                type_resolver,
            )
        }
        Vue3PackageResolutionPhase::JavaScript => {
            resolve_vue3_metadata_legacy_package_javascript_field_path(
                &candidate.join("index.js"),
                Vue3PackageTargetPathPolicy::RequireExplicitFileName,
                type_resolver,
            )
        }
    }
}

fn vue3_package_types_versions_target_policy(
    subpath: Option<&str>,
    module_type: Vue3PackageModuleType,
    exports: Option<&serde_json::Value>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageTypeTargetPolicy {
    if subpath.is_none() {
        return match vue3_package_root_field_target_policy(
            module_type,
            resolution_mode,
            type_resolver,
        ) {
            Vue3PackageTargetPathPolicy::AllowImplicit => {
                Vue3PackageTypeTargetPolicy::AllowImplicit
            }
            Vue3PackageTargetPathPolicy::RequireExplicitFileName => {
                Vue3PackageTypeTargetPolicy::RequireExplicitFileName
            }
        };
    }
    if !type_resolver
        .module_resolution
        .uses_node_esm_specifier_rules(resolution_mode, &type_resolver.typescript_version)
    {
        return Vue3PackageTypeTargetPolicy::AllowImplicit;
    }
    if type_resolver.typescript_version < (5, 8, 0).into()
        && exports.is_none_or(serde_json::Value::is_null)
    {
        Vue3PackageTypeTargetPolicy::RequireExplicitFileNameWithLegacyIndexJsFallback
    } else {
        Vue3PackageTypeTargetPolicy::RequireExplicitFileName
    }
}

pub(crate) fn vue3_package_types_versions_mapping<'a>(
    types_versions: &'a Vue3PackageTypesVersions,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<&'a Vue3PackageTypesVersionMappings> {
    for entry in &types_versions.0 {
        if !type_resolver
            .external_type_session
            .claim_metadata_match_steps(entry.selector.len())
        {
            return None;
        }
        if vue3_package_types_version_selector_matches_version(
            &entry.selector,
            &type_resolver.typescript_version,
        ) {
            return match &entry.value {
                Vue3PackageTypesVersionValue::Unavailable => None,
                Vue3PackageTypesVersionValue::Mappings(mappings) => Some(mappings),
            };
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn vue3_package_types_version_selector_matches(selector: &str) -> bool {
    vue3_package_types_version_selector_matches_version(
        selector,
        &vue3_package_typescript_baseline_version(),
    )
}

pub(crate) fn vue3_package_types_version_selector_matches_version(
    selector: &str,
    typescript_version: &nodejs_semver::Version,
) -> bool {
    let selector = selector.trim();
    if selector.is_empty() {
        return false;
    }
    nodejs_semver::Range::parse(selector).is_ok_and(|range| range.satisfies(typescript_version))
}

pub(crate) fn vue3_package_typescript_baseline_version() -> nodejs_semver::Version {
    // Bounded SFC resolver baseline for the locked Vue 3 compiler-sfc harness.
    (5, 0, 0).into()
}

fn vue3_package_type_field_path_with_mode(
    package_dir: &Path,
    target: &str,
    resolution_mode: Vue3TypeResolutionMode,
    policy: Vue3PackageTargetPathPolicy,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !vue3_package_type_target_is_safe(target) {
        return None;
    }
    let candidate =
        vue3_package_type_target_candidate(package_dir, target.trim_start_matches("./"), type_resolver)?;
    resolve_vue3_metadata_legacy_package_type_field_path_with_mode(
        &candidate,
        resolution_mode,
        policy,
        type_resolver,
    )
}

fn vue3_package_main_javascript_path(
    package_dir: &Path,
    target: &str,
    policy: Vue3PackageTargetPathPolicy,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let candidate =
        vue3_package_type_target_candidate(package_dir, target.trim_start_matches("./"), type_resolver)?;
    resolve_vue3_metadata_legacy_package_javascript_field_path(
        &candidate,
        policy,
        type_resolver,
    )
}

fn vue3_package_root_field_target_policy(
    module_type: Vue3PackageModuleType,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageTargetPathPolicy {
    if module_type == Vue3PackageModuleType::Module
        && type_resolver
            .module_resolution
            .uses_node_esm_specifier_rules(resolution_mode, &type_resolver.typescript_version)
    {
        Vue3PackageTargetPathPolicy::RequireExplicitFileName
    } else {
        Vue3PackageTargetPathPolicy::AllowImplicit
    }
}

pub(crate) fn vue3_package_type_target_is_safe(target: &str) -> bool {
    if target.is_empty() || target.contains(':') || Path::new(target).is_absolute() {
        return false;
    }
    let mut has_normal = false;
    for component in Path::new(target).components() {
        match component {
            std::path::Component::Normal(_) => has_normal = true,
            std::path::Component::CurDir => {}
            _ => return false,
        }
    }
    has_normal
}

fn vue3_package_type_target_candidate(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if !type_resolver
        .external_type_session
        .metadata_path_is_within_limit(target)
    {
        return None;
    }
    if !vue3_package_type_target_is_safe(target) {
        return None;
    }
    Some(normalize_path_components(package_dir.join(target)))
}
