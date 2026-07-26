#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PackageJsonTypeResolution {
    NoPackageJson,
    NoPackageTypeEntry,
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
                                _ => Vue3PackageModuleType::CommonJs,
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

#[cfg(test)]
pub(crate) fn resolve_vue3_package_json_type_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    resolve_vue3_package_json_type_entry_with_mode(
        package_dir,
        subpath,
        Vue3TypeResolutionMode::Import,
        type_resolver,
    )
}

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
    )
}

pub(crate) fn resolve_vue3_package_json_type_reference_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    resolution_mode: Option<Vue3TypeResolutionMode>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    resolve_vue3_package_json_type_entry_with_exports(
        package_dir,
        subpath,
        type_resolver,
        resolution_mode,
        true,
    )
}

fn resolve_vue3_package_json_type_entry_with_exports(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
    exports_mode: Option<Vue3TypeResolutionMode>,
    declaration_only_exports: bool,
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
    if let Some((exports, resolution_mode)) = manifest
        .exports
        .as_ref()
        .zip(exports_mode)
        .filter(|(exports, _)| !exports.is_null())
    {
        return resolve_vue3_package_exports_type_path(
            package_dir,
            exports,
            subpath,
            resolution_mode,
            declaration_only_exports,
            type_resolver,
        );
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
            .or_else(|| {
                manifest
                    .main
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
            })
    } else {
        None
    };
    let path_resolution_mode = exports_mode.unwrap_or(Vue3TypeResolutionMode::Import);
    let types_versions_resolution = vue3_package_types_versions_type_path(
        package_dir,
        &manifest.types_versions,
        subpath,
        root_type_target,
        path_resolution_mode,
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
            let resolved = vue3_package_type_field_path_with_mode(
                package_dir,
                target,
                path_resolution_mode,
                type_resolver,
            );
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
    Rejected,
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
                vue3_package_export_type_path_with_mode(
                    package_dir,
                    target,
                    resolution_mode,
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
    if key == "." {
        return if object
            .keys()
            .next()
            .is_none_or(|key| key != "." && !key.starts_with("./"))
        {
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
    }

    let mut selected = None;
    for (pattern, target) in object {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
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
    if source.contains('*') || !vue3_package_import_specifier_is_safe(source) {
        return Vue3PackageTargetVisit::Invalid;
    }
    let Some(object) = imports.as_object() else {
        return Vue3PackageTargetVisit::Invalid;
    };
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
        if !vue3_package_import_expansion_key_is_safe(pattern) {
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
        let prefix_expansion = matches!(expansion, Vue3PackageTargetExpansion::Prefix(_));
        if !vue3_package_target_is_safe(target, target_kind, prefix_expansion) {
            return Vue3PackageTargetVisit::Invalid;
        }
        let expanded = match expansion {
            Vue3PackageTargetExpansion::Exact => {
                if target.contains('*') {
                    return Vue3PackageTargetVisit::Invalid;
                }
                target.to_string()
            }
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
            || !vue3_package_target_is_safe(&expanded, target_kind, false)
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
        return Vue3PackageTargetVisit::Rejected;
    }
    if let Some(targets) = target.as_array() {
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
                Vue3PackageTargetVisit::Missing | Vue3PackageTargetVisit::Invalid => {}
                result => return result,
            }
        }
        return Vue3PackageTargetVisit::Missing;
    }
    let Some(conditions) = target.as_object() else {
        return Vue3PackageTargetVisit::Invalid;
    };
    for (condition, target) in conditions {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return Vue3PackageTargetVisit::Blocked;
        }
        if condition == "." || condition.starts_with("./") {
            return Vue3PackageTargetVisit::Invalid;
        }
        if !vue3_package_export_condition_is_active(
            condition,
            resolution_mode,
            &type_resolver.typescript_version,
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
            result => return result,
        }
    }
    Vue3PackageTargetVisit::Missing
}

fn vue3_package_target_is_safe(
    target: &str,
    target_kind: Vue3PackageTargetKind,
    allow_trailing_slash: bool,
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

pub(crate) fn vue3_package_import_specifier_is_safe(source: &str) -> bool {
    vue3_package_import_specifier_is_safe_with_trailing_slash(source, false)
}

fn vue3_package_import_specifier_is_safe_with_trailing_slash(
    source: &str,
    allow_trailing_slash: bool,
) -> bool {
    let source = if allow_trailing_slash {
        source.strip_suffix('/').unwrap_or(source)
    } else {
        source
    };
    if source == "#"
        || !source.starts_with('#')
        || source.starts_with("#/")
        || source.ends_with('/')
        || source.contains('\\')
        || vue3_package_export_contains_encoded_separator(source)
    {
        return false;
    }
    let body = &source[1..];
    !body.is_empty()
        && body
            .split('/')
            .all(|segment| !segment.is_empty() && !vue3_package_export_segment_is_forbidden(segment))
}

fn vue3_package_import_expansion_key_is_safe(key: &str) -> bool {
    key.bytes().filter(|byte| *byte == b'*').count() <= 1
        && vue3_package_import_specifier_is_safe_with_trailing_slash(key, true)
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
    typescript_version: &nodejs_semver::Version,
) -> bool {
    condition == "types"
        || condition == "node"
        || condition == "default"
        || matches!(
            (condition, resolution_mode),
            ("import", Vue3TypeResolutionMode::Import)
                | ("require", Vue3TypeResolutionMode::Require)
        )
        || condition.strip_prefix("types@").is_some_and(|selector| {
            vue3_package_types_version_selector_matches_version(selector, typescript_version)
        })
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

fn vue3_package_export_type_path_with_mode(
    package_dir: &Path,
    target: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !target.starts_with("./") {
        return None;
    }
    vue3_package_type_target_path_with_mode(
        package_dir,
        target,
        resolution_mode,
        type_resolver,
    )
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
    resolve_vue3_metadata_type_reference_declaration_file(&candidate, type_resolver)
}

pub(crate) fn vue3_package_types_versions_type_path(
    package_dir: &Path,
    types_versions: &Vue3PackageTypesVersions,
    subpath: Option<&str>,
    root_type_target: Option<&str>,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let mappings = vue3_package_types_versions_mapping(types_versions, type_resolver)?;
    let source = subpath
        .map(|subpath| subpath.trim_start_matches("./").to_string())
        .or_else(|| root_type_target.map(|target| target.trim_start_matches("./").to_string()))
        .unwrap_or_else(|| "index.d.ts".to_string());
    let (mapping_index, capture) = vue3_typescript_best_path_pattern_match(
        mappings
            .0
            .iter()
            .enumerate()
            .map(|(index, (pattern, _))| (index, pattern.as_str())),
        &source,
    )?;
    let targets = &mappings.0[mapping_index].1;
    for target in vue3_tsconfig_path_target_values(targets) {
        if !type_resolver
            .external_type_session
            .claim_metadata_fanout_entry()
        {
            return None;
        }
        let target =
            vue3_typescript_path_target_substitution(&target, &capture, type_resolver)?;
        if !vue3_package_type_target_is_safe(&target) {
            type_resolver.external_type_session.block_metadata();
            return None;
        }
        let resolved = vue3_package_type_field_path_with_mode(
            package_dir,
            &target,
            resolution_mode,
            type_resolver,
        );
        if type_resolver.external_type_session.metadata_is_blocked() {
            return None;
        }
        if let Some(resolved) = resolved {
            return Some(resolved);
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

fn vue3_package_type_field_path_with_mode(
    package_dir: &Path,
    target: &str,
    resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !vue3_package_type_target_is_safe(target) {
        return None;
    }
    vue3_package_type_target_path_with_mode(
        package_dir,
        target.trim_start_matches("./"),
        resolution_mode,
        type_resolver,
    )
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

fn vue3_package_type_target_path_with_mode(
    package_dir: &Path,
    target: &str,
    resolution_mode: Vue3TypeResolutionMode,
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
    let candidate = normalize_path_components(package_dir.join(target));
    resolve_vue3_metadata_type_import_path_with_mode(&candidate, resolution_mode, type_resolver)
}
