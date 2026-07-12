#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PackageJsonTypeResolution {
    NoPackageJson,
    NoPackageTypeEntry,
    Resolved(PathBuf),
    Blocked,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Vue3PackageJsonTypeManifest {
    pub(crate) version: Option<serde_json::Value>,
    pub(crate) tsconfig: Option<serde_json::Value>,
    pub(crate) exports: Option<serde_json::Value>,
    pub(crate) types: Option<serde_json::Value>,
    pub(crate) typings: Option<serde_json::Value>,
    pub(crate) types_versions: Vue3PackageTypesVersions,
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
                        "version" => manifest.version = Some(map.next_value()?),
                        "tsconfig" => manifest.tsconfig = Some(map.next_value()?),
                        "exports" => manifest.exports = Some(map.next_value()?),
                        "types" => manifest.types = Some(map.next_value()?),
                        "typings" => manifest.typings = Some(map.next_value()?),
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
    pub(crate) selector: String,
    pub(crate) mappings: Vue3PackageTypesVersionMappings,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Vue3PackageTypesVersionMappings(Vec<(String, serde_json::Value)>);

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
                let mut entries = Vec::new();
                while let Some(selector) = map.next_key::<String>()? {
                    let mappings = map.next_value::<Vue3PackageTypesVersionMappings>()?;
                    if !mappings.0.is_empty() {
                        entries.push(Vue3PackageTypesVersionEntry { selector, mappings });
                    }
                }
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

impl<'de> Deserialize<'de> for Vue3PackageTypesVersionMappings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TypesVersionMappingsVisitor;

        impl<'de> Visitor<'de> for TypesVersionMappingsVisitor {
            type Value = Vue3PackageTypesVersionMappings;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package.json typesVersions mapping object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut mappings = Vec::new();
                while let Some(pattern) = map.next_key::<String>()? {
                    mappings.push((pattern, map.next_value()?));
                }
                Ok(Vue3PackageTypesVersionMappings(mappings))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Vue3PackageTypesVersionMappings::default())
            }
        }

        deserializer.deserialize_any(TypesVersionMappingsVisitor)
    }
}

pub(crate) fn resolve_vue3_package_json_type_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageJsonTypeResolution::Blocked;
    }
    let Some(_resolution_guard) = type_resolver
        .external_type_session
        .begin_package_resolution(package_dir)
    else {
        return Vue3PackageJsonTypeResolution::Blocked;
    };
    let package_json = package_dir.join("package.json");
    let Some(manifest) = type_resolver
        .external_type_session
        .package_json_from_path(&package_json)
    else {
        return if type_resolver.external_type_session.metadata_is_blocked() {
            Vue3PackageJsonTypeResolution::Blocked
        } else {
            Vue3PackageJsonTypeResolution::NoPackageJson
        };
    };
    if let Some(exports) = &manifest.exports {
        if let Some(target) = vue3_package_exports_type_target(exports, subpath) {
            let resolved = vue3_package_export_type_path(package_dir, &target, type_resolver);
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vue3PackageJsonTypeResolution::Blocked;
            }
            if let Some(resolved) = resolved {
                return Vue3PackageJsonTypeResolution::Resolved(resolved);
            }
            return Vue3PackageJsonTypeResolution::Blocked;
        }
        if subpath.is_some() {
            return Vue3PackageJsonTypeResolution::Blocked;
        }
    }
    let root_type_target = if subpath.is_none() {
        manifest
            .types
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                manifest
                    .typings
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
            })
    } else {
        None
    };
    let types_versions_resolution = vue3_package_types_versions_type_path(
        package_dir,
        &manifest.types_versions,
        subpath,
        root_type_target,
        type_resolver,
    );
    if type_resolver.external_type_session.metadata_is_blocked() {
        return Vue3PackageJsonTypeResolution::Blocked;
    }
    if let Some(resolved) = types_versions_resolution {
        return Vue3PackageJsonTypeResolution::Resolved(resolved);
    }
    if subpath.is_none() {
        if let Some(target) = root_type_target {
            if !vue3_package_type_target_is_safe(target) {
                return Vue3PackageJsonTypeResolution::Blocked;
            }
            let resolved = vue3_package_type_field_path(package_dir, target, type_resolver);
            if type_resolver.external_type_session.metadata_is_blocked() {
                return Vue3PackageJsonTypeResolution::Blocked;
            }
            if let Some(resolved) = resolved {
                return Vue3PackageJsonTypeResolution::Resolved(resolved);
            }
        }
    }
    if type_resolver.external_type_session.metadata_is_blocked() {
        Vue3PackageJsonTypeResolution::Blocked
    } else {
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry
    }
}

pub(crate) fn vue3_package_exports_type_target(
    exports: &serde_json::Value,
    subpath: Option<&str>,
) -> Option<String> {
    let key = subpath
        .map(|subpath| format!("./{}", subpath.trim_start_matches("./")))
        .unwrap_or_else(|| ".".into());
    let target = if key == "." {
        exports
            .get(".")
            .or_else(|| vue3_package_exports_is_condition_map(exports).then_some(exports))
            .and_then(vue3_package_export_target_value)
    } else {
        exports
            .get(&key)
            .and_then(vue3_package_export_target_value)
            .or_else(|| vue3_package_exports_pattern_target(exports, &key))
    }?;
    Some(target)
}

pub(crate) fn vue3_package_exports_is_condition_map(exports: &serde_json::Value) -> bool {
    exports
        .as_object()
        .is_none_or(|object| !object.keys().any(|key| key == "." || key.starts_with("./")))
}

pub(crate) fn vue3_package_exports_pattern_target(
    exports: &serde_json::Value,
    key: &str,
) -> Option<String> {
    let object = exports.as_object()?;
    for (pattern, target) in object {
        let Some(capture) = vue3_package_export_pattern_capture(pattern, key) else {
            continue;
        };
        let target = vue3_package_export_target_value(target)?;
        return Some(target.replace('*', &capture));
    }
    None
}

pub(crate) fn vue3_package_export_target_value(value: &serde_json::Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_string());
    }
    let object = value.as_object()?;
    for condition in ["types", "typings"] {
        if let Some(target) = object
            .get(condition)
            .and_then(vue3_package_export_target_value)
        {
            return Some(target);
        }
    }
    for condition in ["import", "require", "node", "default"] {
        if let Some(target) = object
            .get(condition)
            .and_then(vue3_package_export_target_value)
        {
            return Some(target);
        }
    }
    None
}

pub(crate) fn vue3_package_export_pattern_capture(pattern: &str, key: &str) -> Option<String> {
    let star = pattern.find('*')?;
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !key.starts_with(prefix) || !key.ends_with(suffix) || key.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some(key[prefix.len()..key.len() - suffix.len()].to_string())
}

pub(crate) fn vue3_package_export_type_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !target.starts_with("./") {
        return None;
    }
    vue3_package_type_target_path(package_dir, target, type_resolver)
}

pub(crate) fn vue3_package_types_versions_type_path(
    package_dir: &Path,
    types_versions: &Vue3PackageTypesVersions,
    subpath: Option<&str>,
    root_type_target: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let mappings = vue3_package_types_versions_mapping(types_versions, type_resolver)?;
    let source = subpath
        .map(|subpath| subpath.trim_start_matches("./").to_string())
        .or_else(|| root_type_target.map(|target| target.trim_start_matches("./").to_string()))
        .unwrap_or_else(|| "index.d.ts".to_string());
    let mut matches = mappings
        .0
        .iter()
        .enumerate()
        .filter_map(|(order, (pattern, targets))| {
            let targets = vue3_tsconfig_path_target_values(targets);
            if targets.is_empty() {
                return None;
            }
            vue3_tsconfig_path_pattern_capture(pattern, &source)
                .map(|(score, capture)| (score, order, capture, targets))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    for (_, _, capture, targets) in matches {
        for target in targets {
            let target = target.replace('*', &capture);
            if !vue3_package_type_target_is_safe(&target) {
                type_resolver.external_type_session.block_metadata();
                return None;
            }
            let resolved = vue3_package_type_field_path(package_dir, &target, type_resolver);
            if type_resolver.external_type_session.metadata_is_blocked() {
                return None;
            }
            if let Some(resolved) = resolved {
                return Some(resolved);
            }
        }
    }
    None
}

pub(crate) fn vue3_package_types_versions_mapping<'a>(
    types_versions: &'a Vue3PackageTypesVersions,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<&'a Vue3PackageTypesVersionMappings> {
    types_versions
        .0
        .iter()
        .find(|entry| {
            vue3_package_types_version_selector_matches_version(
                &entry.selector,
                &type_resolver.typescript_version,
            )
        })
        .map(|entry| &entry.mappings)
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

pub(crate) fn vue3_package_type_field_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !vue3_package_type_target_is_safe(target) {
        return None;
    }
    vue3_package_type_target_path(package_dir, target.trim_start_matches("./"), type_resolver)
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

pub(crate) fn vue3_package_type_target_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if type_resolver.external_type_session.metadata_is_blocked() {
        return None;
    }
    if !vue3_package_type_target_is_safe(target) {
        return None;
    }
    let candidate = normalize_path_components(package_dir.join(target));
    if candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "js" | "jsx" | "mjs" | "cjs"))
    {
        let extension = candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        let stem = candidate.with_extension("");
        if let Some(resolved) = vue3_ts_resolution_candidates(&stem, extension)
            .into_iter()
            .find(|candidate| candidate.exists())
        {
            return Some(resolved);
        }
    }
    resolve_vue3_type_import_path(&candidate, type_resolver)
}
