fn vue3_package_manifest_with_exact_len(len: usize) -> String {
    let empty = r#"{"types":"index.d.ts","padding":""}"#;
    assert!(len >= empty.len());
    format!(
        r#"{{"types":"index.d.ts","padding":"{}"}}"#,
        "x".repeat(len - empty.len())
    )
}

fn write_vue3_test_type_package(package_dir: &Path, manifest: &str) {
    std::fs::create_dir_all(package_dir).expect("create type package");
    std::fs::write(package_dir.join("package.json"), manifest).expect("write package manifest");
    std::fs::write(
        package_dir.join("index.d.ts"),
        "export interface Props { value: string }",
    )
    .expect("write package types");
}

struct Vue3PackageTargetFixture {
    target: &'static str,
    permissive_path: Option<PathBuf>,
    explicit_path: Option<PathBuf>,
}

fn write_vue3_package_target_fixture(
    package_dir: &Path,
    target_kind: &str,
) -> Vue3PackageTargetFixture {
    let mapped = package_dir.join("mapped");
    std::fs::create_dir_all(&mapped).expect("create mapped target directory");
    match target_kind {
        "extensionless" => {
            let path = mapped.join("extensionless.d.ts");
            std::fs::write(&path, "export interface ExtensionlessProps {}")
                .expect("write extensionless target");
            Vue3PackageTargetFixture {
                target: "./mapped/extensionless",
                permissive_path: Some(path),
                explicit_path: None,
            }
        }
        "directory" => {
            let directory = mapped.join("directory");
            std::fs::create_dir_all(&directory).expect("create mapped target directory");
            std::fs::write(
                directory.join("package.json"),
                r#"{"types":"nested.d.ts"}"#,
            )
            .expect("write nested target manifest decoy");
            std::fs::write(
                directory.join("nested.d.ts"),
                "export interface WrongNestedManifestProps {}",
            )
            .expect("write nested target manifest entry");
            let path = directory.join("index.d.ts");
            std::fs::write(&path, "export interface DirectoryIndexProps {}")
                .expect("write mapped directory index");
            Vue3PackageTargetFixture {
                target: "./mapped/directory",
                permissive_path: Some(path),
                explicit_path: None,
            }
        }
        "explicit" => {
            let path = mapped.join("explicit.d.ts");
            std::fs::write(&path, "export interface ExplicitProps {}")
                .expect("write explicit target");
            Vue3PackageTargetFixture {
                target: "./mapped/explicit.js",
                permissive_path: Some(path.clone()),
                explicit_path: Some(path),
            }
        }
        "appended" => {
            let path = mapped.join("appended.js.d.ts");
            std::fs::write(&path, "export interface AppendedProps {}")
                .expect("write appended target");
            Vue3PackageTargetFixture {
                target: "./mapped/appended.js",
                permissive_path: Some(path),
                explicit_path: None,
            }
        }
        "arbitrary-declaration" => {
            let path = mapped.join("styles.d.css.ts");
            std::fs::write(&path, "export interface StyleProps {}")
                .expect("write arbitrary declaration target");
            Vue3PackageTargetFixture {
                target: "./mapped/styles.css",
                permissive_path: Some(path.clone()),
                explicit_path: Some(path),
            }
        }
        "raw-javascript" => {
            let path = mapped.join("raw.js");
            std::fs::write(&path, "export const implementationTarget = true;")
                .expect("write raw JavaScript target");
            Vue3PackageTargetFixture {
                target: "./mapped/raw.js",
                permissive_path: Some(path.clone()),
                explicit_path: Some(path),
            }
        }
        _ => unreachable!(),
    }
}

fn vue3_package_resolution_path(
    resolution: Vue3PackageJsonTypeResolution,
) -> Option<PathBuf> {
    match resolution {
        Vue3PackageJsonTypeResolution::Resolved(path) => Some(path),
        Vue3PackageJsonTypeResolution::Blocked => panic!("package metadata was blocked"),
        Vue3PackageJsonTypeResolution::NoPackageJson
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntry
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntryWithoutIndex
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntryWithoutNestedManifest => None,
    }
}

fn vue3_node_next_type_resolver() -> Vue3TypeResolverContext {
    Vue3TypeResolverContext {
        module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
        ..Vue3TypeResolverContext::default()
    }
}

fn vue3_node_next_type_resolver_with_external_limits(
    limits: Vue3ExternalTypeLoadLimits,
) -> Vue3TypeResolverContext {
    Vue3TypeResolverContext {
        module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
        ..Vue3TypeResolverContext::default()
    }
}

#[test]
fn vue3_generated_metadata_paths_are_bounded_before_expansion() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES, 64 * 1024);
    assert_eq!(
        vue3_bounded_replace("*/*", "*", "123456789", 19).as_deref(),
        Some("123456789/123456789")
    );
    assert!(vue3_bounded_replace("*/*", "*", "123456789", 18).is_none());
    assert_eq!(
        vue3_bounded_replace_first("*/*", "*", "123456789", 11).as_deref(),
        Some("123456789/*")
    );
    assert!(vue3_bounded_replace_first("*/*", "*", "123456789", 10).is_none());

    let paths_resolver = vue3_type_resolver_with_external_limits(
        Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: 18,
            ..Vue3ExternalTypeLoadLimits::default()
        },
    );
    let mappings = vue3_tsconfig_direct_path_mappings(
        &serde_json::json!({
            "compilerOptions": {
                "paths": { "alias/*": ["*/*"] }
            }
        }),
        Path::new("."),
        Path::new("."),
        &paths_resolver,
    );
    assert!(resolve_vue3_tsconfig_path_mappings(
        &mappings,
        "alias/123456789",
        &paths_resolver,
    )
    .is_none());
    assert!(paths_resolver.external_type_session.metadata_is_blocked());

    let config_dir_resolver = vue3_type_resolver_with_external_limits(
        Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: 18,
            ..Vue3ExternalTypeLoadLimits::default()
        },
    );
    assert!(vue3_tsconfig_include_pattern(
        Path::new("."),
        Path::new("123456789"),
        "${configDir}/${configDir}",
        &config_dir_resolver,
    )
    .is_none());
    assert!(config_dir_resolver
        .external_type_session
        .metadata_is_blocked());
}

#[test]
fn vue3_include_glob_root_honors_exact_generated_path_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let target = format!(r"{}\**\*.d.ts", dir.path().to_string_lossy());
    let normalized_pattern = format!("{}/**/*.d.ts", normalize_path_string(dir.path()));
    let required = normalized_pattern.len();
    let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: required,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        vue3_tsconfig_include_pattern(Path::new("ignored"), Path::new("ignored"), &target, &exact)
            .as_deref(),
        Some(normalized_pattern.as_str())
    );
    assert_eq!(
        vue3_tsconfig_include_root_path(
            Path::new("ignored"),
            Path::new("ignored"),
            &target,
            &exact,
        ),
        Some(dir.path().to_path_buf())
    );
    assert!(!exact.external_type_session.metadata_is_blocked());
    assert_eq!(exact.external_type_session.stats().tsconfig_discovery_entries, 0);

    let short = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: required - 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(vue3_tsconfig_include_root_path(
        Path::new("ignored"),
        Path::new("ignored"),
        &target,
        &short,
    )
    .is_none());
    assert!(short.external_type_session.metadata_is_blocked());
    assert_eq!(short.external_type_session.stats().tsconfig_discovery_entries, 0);
}

#[test]
fn vue3_include_glob_match_work_is_shared_and_fail_closed() {
    assert_eq!(
        VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_GLOB_MATCH_STEPS,
        16 * 1024 * 1024
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types");
    std::fs::create_dir_all(&types).expect("create include directory");
    let first = types.join("first.d.ts");
    let second = types.join("second.d.ts");
    std::fs::write(&first, "declare interface FirstGlobal {}").expect("write first global");
    std::fs::write(&second, "declare interface SecondGlobal {}").expect("write second global");
    let target = format!("{}/**/*.d.ts", normalize_path_string(&types));

    let measuring = Vue3TypeResolverContext::default();
    let measured = vue3_tsconfig_include_global_type_files(
        dir.path(),
        dir.path(),
        &target,
        &measuring,
    );
    assert_eq!(
        measured.into_iter().collect::<BTreeSet<_>>(),
        [first.clone(), second.clone()].into_iter().collect()
    );
    let required = measuring
        .external_type_session
        .stats()
        .tsconfig_glob_match_steps;
    assert!(required > 1);

    let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_glob_match_steps: required,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_include_global_type_files(dir.path(), dir.path(), &target, &exact)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        [first, second].into_iter().collect()
    );
    assert_eq!(
        exact
            .external_type_session
            .stats()
            .tsconfig_glob_match_steps,
        required
    );
    assert!(!exact.external_type_session.metadata_is_blocked());

    let short = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_glob_match_steps: required - 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(vue3_tsconfig_include_global_type_files(dir.path(), dir.path(), &target, &short)
        .is_empty());
    assert_eq!(
        short
            .external_type_session
            .stats()
            .tsconfig_glob_match_steps,
        required - 1
    );
    assert!(short.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_tsconfig_exclude_entries_and_matching_are_bounded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let types = dir.path().join("types");
    std::fs::create_dir_all(&types).expect("create exclude budget fixture");
    let kept = types.join("kept.d.ts");
    let excluded = types.join("excluded.d.ts");
    std::fs::write(&kept, "declare interface Kept {}").expect("write kept declaration");
    std::fs::write(&excluded, "declare interface Excluded {}")
        .expect("write excluded declaration");
    let value = serde_json::json!({
        "include": ["./types/**/*.d.ts"],
        "exclude": ["./types/excluded.d.ts"],
        "compilerOptions": { "types": [] }
    });

    let measured = Vue3TypeResolverContext::default();
    assert_eq!(
        vue3_tsconfig_direct_global_type_files(&value, dir.path(), dir.path(), &measured),
        vec![kept.clone()]
    );
    let stats = measured.external_type_session.stats();
    let required_entries = stats.tsconfig_discovery_entries;
    let required_files = stats.tsconfig_discovery_files;
    let required_steps = stats.tsconfig_glob_match_steps;
    assert!(required_entries >= 2);
    assert_eq!(required_files, 2);
    assert!(required_steps > 0);

    let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: required_entries,
        max_tsconfig_discovery_files: required_files,
        max_tsconfig_glob_match_steps: required_steps,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_direct_global_type_files(&value, dir.path(), dir.path(), &exact),
        vec![kept]
    );
    assert!(!exact.external_type_session.metadata_is_blocked());

    for limits in [
        Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_entries: required_entries - 1,
            max_tsconfig_discovery_files: required_files,
            max_tsconfig_glob_match_steps: required_steps,
            ..Vue3ExternalTypeLoadLimits::default()
        },
        Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_entries: required_entries,
            max_tsconfig_discovery_files: required_files,
            max_tsconfig_glob_match_steps: required_steps - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        },
    ] {
        let resolver = vue3_type_resolver_with_external_limits(limits);
        assert!(vue3_tsconfig_direct_global_type_files(
            &value,
            dir.path(),
            dir.path(),
            &resolver,
        )
        .is_empty());
        assert!(resolver.external_type_session.metadata_is_blocked());
    }
}

#[test]
fn vue3_adversarial_include_globs_stop_at_the_work_limit() {
    let max_path_bytes = VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES;
    let pattern = format!("*{}b", "a".repeat(max_path_bytes - 2));
    let path = "a".repeat(max_path_bytes - 1);
    assert_eq!(pattern.len(), max_path_bytes);
    assert_eq!(path.len(), max_path_bytes - 1);

    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_glob_match_steps: 1_000,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_glob_matches_with_session(
            &pattern,
            &path,
            &resolver.external_type_session,
        ),
        None
    );
    assert_eq!(
        resolver
            .external_type_session
            .stats()
            .tsconfig_glob_match_steps,
        1_000
    );
    assert!(resolver.external_type_session.metadata_is_blocked());

    let zero = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_glob_match_steps: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_glob_matches_with_session("*", "value", &zero.external_type_session),
        None
    );
    assert_eq!(
        zero.external_type_session
            .stats()
            .tsconfig_glob_match_steps,
        0
    );
    assert!(zero.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_config_dir_template_expansion_is_prefix_only_and_bounded() {
    let template_config_dir = Path::new("expanded-config-directory");
    let resolver = Vue3TypeResolverContext::default();

    assert_eq!(
        vue3_tsconfig_expand_config_dir_template(
            "literal/${configDir}/types",
            template_config_dir,
            &resolver,
        )
        .as_deref(),
        Some("literal/${configDir}/types")
    );

    let target = "${configDir}/types/${configDir}/leaf";
    let expanded = "expanded-config-directory/types/${configDir}/leaf";
    assert_eq!(
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, &resolver).as_deref(),
        Some(expanded)
    );
    assert_eq!(
        vue3_tsconfig_path_mapping_target_path(
            Path::new("mapping-base"),
            template_config_dir,
            "*",
            "${configDir}/captured",
            &resolver,
        ),
        Some(
            Path::new("mapping-base")
                .join("${configDir}")
                .join("captured")
        )
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: expanded.len(),
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, &accepted).as_deref(),
        Some(expanded)
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: expanded.len() - 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(
        vue3_tsconfig_expand_config_dir_template(target, template_config_dir, &rejected).is_none()
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
    assert!(vue3_tsconfig_expand_config_dir_template(
        "${configDir}/ok",
        Path::new("safe"),
        &rejected,
    )
    .is_none());
}

#[test]
fn vue3_tsconfig_filesystem_paths_normalize_separators_and_bound_joined_paths() {
    let resolver = Vue3TypeResolverContext::default();
    assert!(vue3_tsconfig_path_is_relative(r".\config\base.json"));
    assert!(vue3_tsconfig_path_is_relative(r"..\config\base.json"));
    assert_eq!(
        vue3_normalize_typescript_path_separators(r".\types\*.d.ts", &resolver).as_deref(),
        Some("./types/*.d.ts")
    );
    let escaped =
        vue3_normalize_typescript_path_separators(r"..\outside.d.ts", &resolver)
            .expect("normalize package mapping target");
    assert_eq!(escaped, "../outside.d.ts");
    assert!(!vue3_package_type_target_is_safe(&escaped));

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: "base/leaf".len(),
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_target_path(Path::new("base"), Path::new("base"), "leaf", &accepted),
        Some(PathBuf::from("base/leaf"))
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: "base/leaf".len() - 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(
        vue3_tsconfig_target_path(Path::new("base"), Path::new("base"), "leaf", &rejected)
            .is_none()
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_typescript_path_kinds_are_classified_independently_of_the_host() {
    assert_eq!(
        vue3_typescript_path_kind("types/index.d.ts"),
        Vue3TypeScriptPathKind::Relative
    );
    assert_eq!(
        vue3_typescript_path_kind("/types/index.d.ts"),
        Vue3TypeScriptPathKind::Rooted
    );
    for target in ["C:/types/index.d.ts", "z:/types/index.d.ts"] {
        assert_eq!(
            vue3_typescript_path_kind(target),
            Vue3TypeScriptPathKind::WindowsDriveAbsolute
        );
    }
    assert_eq!(
        vue3_typescript_path_kind("//server/share/types/index.d.ts"),
        Vue3TypeScriptPathKind::WindowsUncAbsolute
    );
    for target in [
        "C:types/index.d.ts",
        "1:/types/index.d.ts",
        "//server",
        "///share/types",
        "//?/C:/types/index.d.ts",
        "//./C:/types/index.d.ts",
        "//server/*/types/index.d.ts",
        "//server/sh:are/types/index.d.ts",
    ] {
        assert_eq!(
            vue3_typescript_path_kind(target),
            Vue3TypeScriptPathKind::Unsupported,
            "{target}"
        );
    }

    assert_eq!(
        vue3_materialize_normalized_typescript_path(
            Path::new("project/config"),
            "../types/index.d.ts",
        ),
        Some(PathBuf::from("project/types/index.d.ts"))
    );
    assert!(vue3_materialize_normalized_typescript_path(
        Path::new("project/config"),
        "C:types/index.d.ts",
    )
    .is_none());
}

#[cfg(windows)]
#[test]
fn vue3_typescript_rooted_paths_materialize_with_windows_volume_semantics() {
    assert_eq!(
        normalize_path_string(
            &vue3_materialize_normalized_typescript_path(
                Path::new("C:/project/config"),
                "/types/index.d.ts",
            )
            .expect("materialize current-volume rooted path"),
        ),
        "C:/types/index.d.ts"
    );
    assert_eq!(
        normalize_path_string(
            &vue3_materialize_normalized_typescript_path(
                Path::new("D:/project/config"),
                "C:/types/index.d.ts",
            )
            .expect("materialize drive-absolute path"),
        ),
        "C:/types/index.d.ts"
    );
    assert_eq!(
        normalize_path_string(
            &vue3_materialize_normalized_typescript_path(
                Path::new("C:/project/config"),
                "//server/share/types/index.d.ts",
            )
            .expect("materialize UNC path"),
        ),
        "//server/share/types/index.d.ts"
    );
}

#[cfg(not(windows))]
#[test]
fn vue3_foreign_windows_paths_do_not_fall_back_to_relative_paths() {
    let base = Path::new("project/config");
    assert!(vue3_materialize_normalized_typescript_path(base, "C:/types/index.d.ts").is_none());
    assert!(vue3_materialize_normalized_typescript_path(
        base,
        "//server/share/types/index.d.ts",
    )
    .is_none());

    let resolver = Vue3TypeResolverContext::default();
    assert!(vue3_tsconfig_target_path(
        base,
        base,
        "C:/types/index.d.ts",
        &resolver,
    )
    .is_none());
    assert!(vue3_resolve_tsconfig_extends_path(
        base,
        "C:/configs/base.json",
        &resolver,
    )
    .is_none());
    assert_eq!(
        resolver
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        0
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_tsconfig_plain_paths_preserve_stars_and_mappings_replace_only_the_first() {
    let resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        vue3_tsconfig_target_path(
            Path::new("config"),
            Path::new("config"),
            "literal*/types",
            &resolver,
        ),
        Some(PathBuf::from("config/literal*/types"))
    );
    assert_eq!(
        vue3_tsconfig_path_mapping_target_path(
            Path::new("config"),
            Path::new("config"),
            "first*second*third",
            "capture",
            &resolver,
        ),
        Some(PathBuf::from("config/firstcapturesecond*third"))
    );
    assert_eq!(
        vue3_tsconfig_path_mapping_target_path(
            Path::new("config"),
            Path::new("config"),
            "empty*capture",
            "",
            &resolver,
        ),
        Some(PathBuf::from("config/empty*capture"))
    );
}

#[test]
fn vue3_types_versions_select_only_the_longest_prefix_pattern() {
    let dir = tempfile::tempdir().expect("temp dir");
    let priority_package = dir.path().join("priority-package");
    write_vue3_test_type_package(
        &priority_package,
        r#"{
            "typesVersions": {
                "*": {
                    "a*bcd": ["wrong.d.ts"],
                    "ab*": ["right.d.ts"]
                }
            }
        }"#,
    );
    std::fs::write(
        priority_package.join("wrong.d.ts"),
        "export interface Wrong {}",
    )
    .expect("write total-length decoy");
    let right = priority_package.join("right.d.ts");
    std::fs::write(&right, "export interface Right {}")
        .expect("write longest-prefix target");
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &priority_package,
            Some("ab-value-bcd"),
            &Vue3TypeResolverContext::default(),
        ),
        Vue3PackageJsonTypeResolution::Resolved(right)
    );

    let fallback_package = dir.path().join("fallback-package");
    write_vue3_test_type_package(
        &fallback_package,
        r#"{
            "typesVersions": {
                "*": {
                    "*": ["weak.d.ts"],
                    "feature-*": ["missing.d.ts"]
                }
            }
        }"#,
    );
    std::fs::write(
        fallback_package.join("weak.d.ts"),
        "export interface Weak {}",
    )
    .expect("write weaker pattern decoy");
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &fallback_package,
            Some("feature-value"),
            &Vue3TypeResolverContext::default(),
        ),
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry
    );

    let exact_package = dir.path().join("exact-package");
    write_vue3_test_type_package(
        &exact_package,
        r#"{
            "typesVersions": {
                "*": {
                    "feature": ["literal*.d.ts"],
                    "*": ["weak.d.ts"]
                }
            }
        }"#,
    );
    std::fs::write(
        exact_package.join("literal.d.ts"),
        "export interface RemovedStarDecoy {}",
    )
    .expect("write removed-star decoy");
    std::fs::write(
        exact_package.join("weak.d.ts"),
        "export interface WeakExactFallback {}",
    )
    .expect("write exact fallback decoy");
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &exact_package,
            Some("feature"),
            &Vue3TypeResolverContext::default(),
        ),
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry
    );
}

#[test]
fn vue3_types_versions_keep_the_first_matching_selector() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source_dir = dir.path().join("src");
    let package = dir
        .path()
        .join("node_modules")
        .join("vuec-first-types-version");
    let feature_dir = package.join("feature");
    std::fs::create_dir_all(&source_dir).expect("create importer directory");
    std::fs::create_dir_all(&feature_dir).expect("create package feature directory");
    let importer = source_dir.join("Comp.vue");
    let root = package.join("types.d.ts");
    let feature = feature_dir.join("item.d.ts");
    std::fs::write(&root, "export interface Root { root: string }")
        .expect("write root fallback");
    std::fs::write(&feature, "export interface Feature { feature: string }")
        .expect("write subpath fallback");
    std::fs::write(
        package.join("wrong.d.ts"),
        "export interface Wrong { wrong: never }",
    )
    .expect("write later selector decoy");

    for (name, manifest) in [
        (
            "empty-object",
            r#"{"types":"types.d.ts","typesVersions":{"*":{},">=0":{"*":["wrong.d.ts"]}}}"#,
        ),
        (
            "number",
            r#"{"types":"types.d.ts","typesVersions":{"*":42,">=0":{"*":["wrong.d.ts"]}}}"#,
        ),
        (
            "array",
            r#"{"types":"types.d.ts","typesVersions":{"*":[],">=0":{"*":["wrong.d.ts"]}}}"#,
        ),
        (
            "null",
            r#"{"types":"types.d.ts","typesVersions":{"*":null,">=0":{"*":["wrong.d.ts"]}}}"#,
        ),
    ] {
        std::fs::write(package.join("package.json"), manifest)
            .expect("write typesVersions manifest");
        let resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            resolve_vue3_type_import(
                &importer.to_string_lossy(),
                "vuec-first-types-version",
                &resolver,
            ),
            Some(root.clone()),
            "root: {name}"
        );
        assert_eq!(
            resolve_vue3_type_import(
                &importer.to_string_lossy(),
                "vuec-first-types-version/feature/item",
                &resolver,
            ),
            Some(feature.clone()),
            "subpath: {name}"
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }
}

#[test]
fn vue3_types_versions_follow_javascript_property_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package = dir.path().join("ordered-types-versions");
    std::fs::create_dir_all(&package).expect("create ordered typesVersions package");
    for name in [
        "wildcard.d.ts",
        "numeric.d.ts",
        "stale.d.ts",
        "replacement.d.ts",
    ] {
        std::fs::write(package.join(name), "export interface Selected {}").expect("write target");
    }

    for (name, manifest, expected) in [
        (
            "array-index-selector-first",
            r#"{"typesVersions":{"*":{"*":["wildcard.d.ts"]},"5":{"*":["numeric.d.ts"]}}}"#,
            "numeric.d.ts",
        ),
        (
            "last-duplicate-selector-value",
            r#"{"typesVersions":{"*":{"*":["stale.d.ts"]},"*":{"*":["replacement.d.ts"]}}}"#,
            "replacement.d.ts",
        ),
        (
            "last-duplicate-mapping-value",
            r#"{"typesVersions":{"*":{"*":["stale.d.ts"],"*":["replacement.d.ts"]}}}"#,
            "replacement.d.ts",
        ),
    ] {
        std::fs::write(package.join("package.json"), manifest)
            .expect("write ordered typesVersions manifest");
        assert_eq!(
            resolve_vue3_package_json_type_entry(
                &package,
                None,
                &Vue3TypeResolverContext::default(),
            ),
            Vue3PackageJsonTypeResolution::Resolved(package.join(expected)),
            "{name}"
        );
    }
}

#[test]
fn vue3_generated_package_paths_are_bounded_before_expansion() {
    let dir = tempfile::tempdir().expect("temp dir");
    let exports_package = dir.path().join("exports-package");
    write_vue3_test_type_package(
        &exports_package,
        r#"{"exports":{"./feature/*":{"types":"./*/*.d.ts"}}}"#,
    );
    let exports_resolver = vue3_type_resolver_with_external_limits(
        Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: 18,
            ..Vue3ExternalTypeLoadLimits::default()
        },
    );
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &exports_package,
            Some("feature/123456789"),
            &exports_resolver,
        ),
        Vue3PackageJsonTypeResolution::Blocked
    );

    let prefix_exports = serde_json::json!({
        "./feature/": { "types": "./types/" }
    });
    let expanded_prefix = "./types/item.d.ts";
    let prefix_accepted =
        vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: expanded_prefix.len(),
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert_eq!(
        vue3_package_exports_type_target(
            &prefix_exports,
            Some("feature/item.d.ts"),
            &prefix_accepted,
        )
        .as_deref(),
        Some(expanded_prefix)
    );
    let prefix_rejected =
        vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: expanded_prefix.len() - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert!(vue3_package_exports_type_target(
        &prefix_exports,
        Some("feature/item.d.ts"),
        &prefix_rejected,
    )
    .is_none());
    assert!(prefix_rejected
        .external_type_session
        .metadata_is_blocked());

    let versions_package = dir.path().join("versions-package");
    write_vue3_test_type_package(
        &versions_package,
        r#"{"typesVersions":{"*":{"*":["*/*.d.ts"]}}}"#,
    );
    let versions_resolver = vue3_type_resolver_with_external_limits(
        Vue3ExternalTypeLoadLimits {
            max_generated_path_bytes: 18,
            ..Vue3ExternalTypeLoadLimits::default()
        },
    );
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &versions_package,
            Some("123456789"),
            &versions_resolver,
        ),
        Vue3PackageJsonTypeResolution::Blocked
    );
}

#[test]
fn vue3_metadata_loader_honors_byte_boundaries_and_fails_closed() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_METADATA_FILE_BYTES, 1024 * 1024);
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_METADATA_BYTES, 16 * 1024 * 1024);
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    let blocked = dir.path().join("blocked");
    let exact_manifest = vue3_package_manifest_with_exact_len(64);
    write_vue3_test_type_package(&first, &exact_manifest);
    write_vue3_test_type_package(&second, &exact_manifest);
    write_vue3_test_type_package(&blocked, r#"{"types":"index.d.ts"}"#);
    std::fs::write(blocked.join("tsconfig.json"), "{}")
        .expect("write package tsconfig fallback");
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_files: 8,
        max_metadata_file_bytes: 64,
        max_metadata_bytes: 128,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert!(resolve_vue3_package_type_entry(&first, None, &resolver).is_some());
    assert!(resolve_vue3_package_type_entry(&second, None, &resolver).is_some());
    assert!(resolve_vue3_package_type_entry(&blocked, None, &resolver).is_none());
    assert!(resolve_vue3_package_tsconfig_entry(&blocked, None, &resolver).is_none());
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 3);
    assert_eq!(stats.metadata_bytes, 128);

    let oversized = dir.path().join("oversized");
    write_vue3_test_type_package(&oversized, &vue3_package_manifest_with_exact_len(65));
    std::fs::write(oversized.join("tsconfig.json"), "{}")
        .expect("write oversized package tsconfig fallback");
    let rejecting_resolver =
        vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_file_bytes: 64,
            max_metadata_bytes: 128,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert!(resolve_vue3_package_type_entry(&oversized, None, &rejecting_resolver).is_none());
    assert!(
        resolve_vue3_package_tsconfig_entry(&oversized, None, &rejecting_resolver).is_none()
    );
    let stats = rejecting_resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_bytes, 0);

    let invalid = dir.path().join("invalid");
    write_vue3_test_type_package(&invalid, r#"{"types":"index.d.ts"}"#);
    std::fs::write(invalid.join("package.json"), vec![0xff; 10])
        .expect("write invalid UTF-8 manifest");
    let invalid_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(&invalid, None, &invalid_resolver),
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert!(resolve_vue3_package_type_entry(&invalid, None, &invalid_resolver).is_none());
    let stats = invalid_resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_bytes, 10);

    let malformed = dir.path().join("malformed");
    write_vue3_test_type_package(&malformed, "{");
    let malformed_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(&malformed, None, &malformed_resolver),
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert!(resolve_vue3_package_type_entry(&malformed, None, &malformed_resolver).is_none());
    assert_eq!(
        malformed_resolver
            .external_type_session
            .stats()
            .metadata_bytes,
        1
    );

    let missing = dir.path().join("missing-slot");
    std::fs::create_dir_all(&missing).expect("create missing package directory");
    let after_limit = dir.path().join("after-limit");
    write_vue3_test_type_package(&after_limit, r#"{"types":"index.d.ts"}"#);
    let file_limited = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_files: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&missing, None, &file_limited),
        Vue3PackageJsonTypeResolution::NoPackageJson
    );
    assert!(resolve_vue3_package_type_entry(&after_limit, None, &file_limited).is_none());
    assert_eq!(
        file_limited
            .external_type_session
            .stats()
            .metadata_files_read,
        1
    );
}

#[test]
fn vue3_metadata_loader_caches_success_and_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("cached");
    write_vue3_test_type_package(&package_dir, r#"{"types":"index.d.ts"}"#);
    let resolver = Vue3TypeResolverContext::default();
    let first = resolve_vue3_package_json_type_entry(&package_dir, None, &resolver);
    std::fs::remove_file(package_dir.join("package.json")).expect("remove cached manifest");
    let second = resolve_vue3_package_json_type_entry(&package_dir, None, &resolver);
    assert!(matches!(first, Vue3PackageJsonTypeResolution::Resolved(_)));
    assert_eq!(first, second);
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_parse_cache_hits, 1);

    let missing = dir.path().join("missing");
    std::fs::create_dir_all(&missing).expect("create missing package directory");
    let missing_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(&missing, None, &missing_resolver),
        Vue3PackageJsonTypeResolution::NoPackageJson
    );
    write_vue3_test_type_package(&missing, r#"{"types":"index.d.ts"}"#);
    assert_eq!(
        resolve_vue3_package_json_type_entry(&missing, None, &missing_resolver),
        Vue3PackageJsonTypeResolution::NoPackageJson
    );
    let stats = missing_resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_source_cache_hits, 1);
}

#[test]
fn vue3_package_metadata_cache_is_shared_by_consumers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    write_vue3_test_type_package(
        &package_dir,
        concat!(
            r#"{"version":"0.0.0","version":"5.4.3","types":"index.d.ts","#,
            r#""tsconfig":"ignored.json","tsconfig":"base.json"}"#,
        ),
    );
    std::fs::write(package_dir.join("base.json"), "{}").expect("write package tsconfig");
    let resolver = Vue3TypeResolverContext::default();
    let package_json = package_dir.join("package.json");

    assert_eq!(
        vue3_typescript_version_from_package_json(&package_json, &resolver),
        nodejs_semver::Version::parse("5.4.3").ok()
    );
    assert!(matches!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &resolver),
        Vue3PackageJsonTypeResolution::Resolved(_)
    ));
    assert_eq!(
        vue3_package_json_tsconfig_entry(&package_dir, &resolver),
        Some(package_dir.join("base.json"))
    );
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_parse_cache_hits, 2);
}

#[test]
fn vue3_tsconfig_metadata_cache_is_shared_between_paths_and_globals() {
    let dir = tempfile::tempdir().expect("temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("create source directory");
    let alias = dir.path().join("alias.ts");
    let global = dir.path().join("global.d.ts");
    std::fs::write(&alias, "export interface AliasProps { value: string }")
        .expect("write aliased type");
    std::fs::write(&global, "declare interface GlobalProps { global: boolean }")
        .expect("write global type");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{
            "files": ["./global.d.ts"],
            "compilerOptions": { "paths": { "alias": ["./alias.ts"] } }
        }"#,
    )
    .expect("write tsconfig");
    let filename = src.join("Comp.vue");
    let filename = filename.to_string_lossy();
    let resolver = Vue3TypeResolverContext::default();

    assert_eq!(
        resolve_vue3_tsconfig_type_import(&filename, "alias", &resolver),
        Some(alias)
    );
    assert_eq!(
        vue3_tsconfig_global_type_files(&filename, &resolver),
        vec![global]
    );
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_parse_cache_hits, 1);
    assert_eq!(stats.tsconfig_nodes, 1);
}

#[test]
fn vue3_metadata_blocks_only_for_reachable_candidate_manifests() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    write_vue3_test_type_package(&package_dir, r#"{"types":"bad"}"#);
    let bad_package = package_dir.join("bad");
    write_vue3_test_type_package(
        &bad_package,
        &vue3_package_manifest_with_exact_len(65),
    );
    let package_resolver =
        vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_file_bytes: 64,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &package_resolver),
        Vue3PackageJsonTypeResolution::Resolved(bad_package.join("index.d.ts"))
    );
    assert_eq!(
        resolve_vue3_package_type_entry(&package_dir, None, &package_resolver),
        Some(bad_package.join("index.d.ts"))
    );
    assert!(!package_resolver
        .external_type_session
        .metadata_is_blocked());
    assert_eq!(
        package_resolver
            .external_type_session
            .stats()
            .metadata_files_read,
        1
    );

    let project = dir.path().join("project");
    let bad_target = project.join("bad");
    std::fs::create_dir_all(&bad_target).expect("create bad path target");
    std::fs::write(bad_target.join("package.json"), "{").expect("write malformed manifest");
    let good_target = project.join("good.ts");
    std::fs::write(&good_target, "export interface GoodProps { value: string }")
        .expect("write good path target");
    std::fs::write(
        project.join("tsconfig.json"),
        r#"{"compilerOptions":{"paths":{"candidate":["./bad","./good.ts"]}}}"#,
    )
    .expect("write path mapping tsconfig");
    let filename = project.join("Comp.vue").to_string_lossy().to_string();
    let path_resolver = Vue3TypeResolverContext::default();
    assert!(resolve_vue3_tsconfig_type_import(&filename, "candidate", &path_resolver).is_none());
}

#[test]
fn vue3_package_metadata_targets_cannot_escape_package_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    let outside = dir.path().join("outside.d.ts");
    std::fs::write(&outside, "export interface OutsideProps { value: string }")
        .expect("write outside type");
    write_vue3_test_type_package(
        &package_dir,
        r#"{"types":"nested/../../outside.d.ts"}"#,
    );
    let resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &resolver),
        Vue3PackageJsonTypeResolution::Blocked
    );

    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"nested\\..\\..\\outside.d.ts"}"#,
    )
    .expect("replace package manifest with Windows traversal target");
    let windows_traversal_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &package_dir,
            None,
            &windows_traversal_resolver,
        ),
        Vue3PackageJsonTypeResolution::Blocked
    );

    std::fs::write(
        package_dir.join("package.json"),
        r#"{"exports":{".":{"types":"./nested/../../outside.d.ts"}}}"#,
    )
    .expect("replace package manifest");
    let fresh_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &fresh_resolver),
        Vue3PackageJsonTypeResolution::Blocked
    );

    std::fs::write(package_dir.join("package.json"), r#"{"types":"."}"#)
        .expect("replace package manifest with self target");
    let self_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &self_resolver),
        Vue3PackageJsonTypeResolution::Blocked
    );
}

#[test]
fn vue3_package_exports_reject_forbidden_path_segments() {
    for target in [
        "./node_modules/x.d.ts",
        "./Node_Modules/x.d.ts",
        "./no%64e_modules/x.d.ts",
        "./types/./x.d.ts",
        "./types/../x.d.ts",
        "./%2e%2E/x.d.ts",
        "./types%2Fx.d.ts",
        "./types%5cx.d.ts",
        r"./types\..\x.d.ts",
    ] {
        assert!(
            !vue3_package_export_target_is_safe(target),
            "unsafe export target was accepted: {target}"
        );
    }
    for target in [
        "./types/x.d.ts",
        "./node_modules-x/x.d.ts",
        "./prefix-node_modules/x.d.ts",
        "./%252e%252e/x.d.ts",
        "./types//x.d.ts",
        "./node_*/x.d.ts",
    ] {
        assert!(
            vue3_package_export_target_is_safe(target),
            "safe export target was rejected: {target}"
        );
    }
    for capture in [
        "node_modules/x",
        "Node_Modules/x",
        "no%64e_modules/x",
        "%2e%2e/x",
        r"types\..\x",
    ] {
        assert!(
            !vue3_package_export_pattern_capture_is_safe(capture),
            "unsafe export capture was accepted: {capture}"
        );
    }
    assert!(vue3_package_export_pattern_capture_is_safe("modules/x"));
    assert!(vue3_package_export_pattern_capture_is_safe("types%2fx"));

    let fixed_resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: 16,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let fixed = serde_json::json!({ "./*": "./fixed.d.ts" });
    let unused_long_capture = format!("types%2f{}", "x".repeat(128));
    assert_eq!(
        vue3_package_exports_type_target(
            &fixed,
            Some(&unused_long_capture),
            &fixed_resolver,
        )
        .as_deref(),
        Some("./fixed.d.ts")
    );
    assert!(!fixed_resolver
        .external_type_session
        .metadata_is_blocked());

    let expanded = serde_json::json!({ "./*": "./types/*.d.ts" });
    let expanded_resolver = Vue3TypeResolverContext::default();
    assert!(vue3_package_exports_type_target(
        &expanded,
        Some("types%2fx"),
        &expanded_resolver,
    )
    .is_none());
    assert!(!expanded_resolver
        .external_type_session
        .metadata_is_blocked());

    let dir = tempfile::tempdir().expect("temp dir");
    let fixed_package = dir.path().join("fixed-package");
    write_vue3_test_type_package(
        &fixed_package,
        r#"{"exports":{"./fixed/*":"./index.d.ts"}}"#,
    );
    let fixed_package_resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &fixed_package,
            Some("fixed/types%2fx"),
            &fixed_package_resolver,
        ),
        Vue3PackageJsonTypeResolution::Resolved(fixed_package.join("index.d.ts"))
    );
    assert!(!fixed_package_resolver
        .external_type_session
        .metadata_is_blocked());

    let expanded_package = dir.path().join("expanded-package");
    write_vue3_test_type_package(
        &expanded_package,
        r#"{"exports":{"./expanded/*":"./types/*.d.ts"}}"#,
    );
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &expanded_package,
            Some("expanded/types%2fx"),
            &Vue3TypeResolverContext::default(),
        ),
        Vue3PackageJsonTypeResolution::Blocked
    );

    let specific = dir.path().join("specific");
    write_vue3_test_type_package(
        &specific,
        r#"{"exports":{"./*":"./types/broad.d.ts","./pre-*/x":"./types/specific.d.ts"}}"#,
    );
    std::fs::create_dir_all(specific.join("types")).expect("create export types");
    std::fs::write(specific.join("types").join("broad.d.ts"), "export type T = string")
        .expect("write broad export target");
    let resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &specific,
            Some("pre-node_modules/x"),
            &resolver,
        ),
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());

    let explicit = dir.path().join("explicit");
    write_vue3_test_type_package(
        &explicit,
        r#"{"exports":{"./node_modules/exact":"./index.d.ts","./node_modules/*":"./index.d.ts"}}"#,
    );
    for subpath in ["node_modules/exact", "node_modules/feature"] {
        assert_eq!(
            resolve_vue3_package_json_type_entry(&explicit, Some(subpath), &resolver),
            Vue3PackageJsonTypeResolution::Resolved(explicit.join("index.d.ts")),
            "{subpath}"
        );
    }

    let legacy = dir.path().join("legacy");
    write_vue3_test_type_package(&legacy, r#"{"types":"node_modules/x.d.ts"}"#);
    std::fs::create_dir_all(legacy.join("node_modules")).expect("create legacy nested package");
    std::fs::write(
        legacy.join("node_modules").join("x.d.ts"),
        "export type Legacy = string",
    )
    .expect("write legacy nested type");
    assert_eq!(
        resolve_vue3_package_json_type_entry(&legacy, None, &resolver),
        Vue3PackageJsonTypeResolution::Resolved(legacy.join("node_modules").join("x.d.ts"))
    );

    let safe = dir.path().join("safe");
    write_vue3_test_type_package(&safe, r#"{"exports":{".":"./index.d.ts"}}"#);
    assert_eq!(
        resolve_vue3_package_json_type_entry(&safe, None, &resolver),
        Vue3PackageJsonTypeResolution::Resolved(safe.join("index.d.ts"))
    );
}

#[test]
fn vue3_truthy_package_exports_block_legacy_root_fallback() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, manifest) in [
        (
            "excluded-root",
            r#"{"types":"index.d.ts","exports":{".":null}}"#,
        ),
        ("empty-exports", r#"{"types":"index.d.ts","exports":{}}"#),
        (
            "subpath-only",
            r#"{"types":"index.d.ts","exports":{"./feature":"./feature.d.ts"}}"#,
        ),
        (
            "boolean-exports",
            r#"{"types":"index.d.ts","exports":true}"#,
        ),
        (
            "numeric-exports",
            r#"{"types":"index.d.ts","exports":1}"#,
        ),
    ] {
        let package_dir = dir.path().join(name);
        write_vue3_test_type_package(&package_dir, manifest);
        assert_eq!(
            resolve_vue3_package_json_type_entry(
                &package_dir,
                None,
                &Vue3TypeResolverContext::default(),
            ),
            Vue3PackageJsonTypeResolution::Blocked,
            "{name}"
        );
    }

    for (name, manifest) in [
        (
            "mixed-export-keys",
            r#"{"types":"legacy.d.ts","exports":{".":"./index.d.ts","types":"./legacy.d.ts"}}"#,
        ),
        (
            "numeric-export-condition",
            r#"{"types":"legacy.d.ts","exports":{"types":"./index.d.ts","0":"./legacy.d.ts"}}"#,
        ),
    ] {
        let package_dir = dir.path().join(name);
        write_vue3_test_type_package(&package_dir, manifest);
        assert_eq!(
            resolve_vue3_package_json_type_entry(
                &package_dir,
                None,
                &Vue3TypeResolverContext::default(),
            ),
            Vue3PackageJsonTypeResolution::Resolved(package_dir.join("index.d.ts")),
            "{name}"
        );
    }

    for (name, exports) in [
        ("null-exports", serde_json::Value::Null),
        ("false-exports", serde_json::json!(false)),
        ("zero-exports", serde_json::json!(0)),
        ("negative-zero-exports", serde_json::json!(-0.0)),
        ("empty-string-exports", serde_json::json!("")),
    ] {
        let package_dir = dir.path().join(name);
        let manifest = serde_json::json!({ "types": "index.d.ts", "exports": exports });
        write_vue3_test_type_package(&package_dir, &manifest.to_string());
        assert_eq!(
            resolve_vue3_package_json_type_entry(
                &package_dir,
                None,
                &Vue3TypeResolverContext::default(),
            ),
            Vue3PackageJsonTypeResolution::Resolved(package_dir.join("index.d.ts")),
            "{name}"
        );
    }
}

#[test]
fn vue3_package_type_fields_follow_typescript_precedence() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("type-fields");
    std::fs::create_dir_all(&package_dir).expect("create type field package");
    for (name, declaration) in [
        ("typings.d.ts", "export type Selected = 'typings'"),
        ("types.d.ts", "export type Selected = 'types'"),
        ("main.d.ts", "export type Selected = 'main'"),
    ] {
        std::fs::write(package_dir.join(name), declaration).expect("write type field target");
    }

    let cases = [
        (
            "typings-before-types",
            serde_json::json!({
                "typings": "typings.d.ts",
                "types": "types.d.ts",
                "main": "main.js",
            }),
            Vue3PackageJsonTypeResolution::Resolved(package_dir.join("typings.d.ts")),
        ),
        (
            "empty-typings",
            serde_json::json!({
                "typings": "",
                "types": "types.d.ts",
                "main": "main.js",
            }),
            Vue3PackageJsonTypeResolution::Resolved(package_dir.join("types.d.ts")),
        ),
        (
            "non-string-type-fields",
            serde_json::json!({
                "typings": false,
                "types": 0,
                "main": "main.js",
            }),
            Vue3PackageJsonTypeResolution::Resolved(package_dir.join("main.d.ts")),
        ),
        (
            "missing-typings",
            serde_json::json!({
                "typings": "missing.d.ts",
                "types": "types.d.ts",
                "main": "main.js",
            }),
            Vue3PackageJsonTypeResolution::NoPackageTypeEntry,
        ),
        (
            "missing-types",
            serde_json::json!({
                "types": "missing.d.ts",
                "main": "main.js",
            }),
            Vue3PackageJsonTypeResolution::NoPackageTypeEntry,
        ),
        (
            "empty-type-fields",
            serde_json::json!({
                "typings": "",
                "types": "",
                "main": "",
            }),
            Vue3PackageJsonTypeResolution::NoPackageTypeEntry,
        ),
    ];
    for (name, manifest, expected) in cases {
        std::fs::write(package_dir.join("package.json"), manifest.to_string())
            .expect("write type field manifest");
        assert_eq!(
            resolve_vue3_package_json_type_entry(
                &package_dir,
                None,
                &Vue3TypeResolverContext::default(),
            ),
            expected,
            "{name}: {manifest}"
        );
    }
}

#[test]
fn vue3_package_root_fields_follow_package_format_path_rules() {
    let dir = tempfile::tempdir().expect("temp dir");
    for field in ["typings", "types", "main"] {
        for (package_type_name, package_type) in [
            ("unspecified", None),
            ("commonjs", Some("commonjs")),
            ("module", Some("module")),
        ] {
            for target_kind in [
                "extensionless",
                "directory",
                "explicit",
                "appended",
                "arbitrary-declaration",
                "arbitrary-raw",
            ] {
                let package_dir = dir
                    .path()
                    .join(format!("{field}-{package_type_name}-{target_kind}"));
                std::fs::create_dir_all(&package_dir).expect("create root field package");
                let (target, permissive_path, explicit_path) = match target_kind {
                    "extensionless" => {
                        let path = package_dir.join("entry.d.ts");
                        std::fs::write(&path, "export interface ExtensionlessProps {}")
                            .expect("write extensionless root field target");
                        ("./entry", Some(path), None)
                    }
                    "directory" => {
                        let target_dir = package_dir.join("directory");
                        std::fs::create_dir_all(&target_dir)
                            .expect("create root field target directory");
                        std::fs::write(
                            target_dir.join("package.json"),
                            r#"{"types":"nested.d.ts"}"#,
                        )
                        .expect("write nested target manifest decoy");
                        std::fs::write(
                            target_dir.join("nested.d.ts"),
                            "export interface WrongNestedManifestProps {}",
                        )
                        .expect("write nested target manifest entry");
                        let path = target_dir.join("index.d.ts");
                        std::fs::write(&path, "export interface DirectoryIndexProps {}")
                            .expect("write root field directory index");
                        ("./directory", Some(path), None)
                    }
                    "explicit" => {
                        let path = package_dir.join("explicit.d.ts");
                        std::fs::write(&path, "export interface ExplicitProps {}")
                            .expect("write explicit root field target");
                        ("./explicit.js", Some(path.clone()), Some(path))
                    }
                    "appended" => {
                        let path = package_dir.join("appended.js.d.ts");
                        std::fs::write(&path, "export interface AppendedProps {}")
                            .expect("write appended root field target");
                        ("./appended.js", Some(path), None)
                    }
                    "arbitrary-declaration" => {
                        let path = package_dir.join("styles.d.css.ts");
                        std::fs::write(&path, "export interface StyleProps {}")
                            .expect("write arbitrary extension declaration");
                        ("./styles.css", Some(path.clone()), Some(path))
                    }
                    "arbitrary-raw" => {
                        std::fs::write(
                            package_dir.join("raw.css"),
                            "export interface WrongRawProps {}",
                        )
                        .expect("write raw arbitrary extension decoy");
                        ("./raw.css", None, None)
                    }
                    _ => unreachable!(),
                };
                let mut manifest = serde_json::Map::new();
                manifest.insert(field.to_string(), serde_json::json!(target));
                if let Some(package_type) = package_type {
                    manifest.insert("type".to_string(), serde_json::json!(package_type));
                }
                std::fs::write(
                    package_dir.join("package.json"),
                    serde_json::Value::Object(manifest).to_string(),
                )
                .expect("write root field package manifest");

                for module_resolution in [
                    Vue3TypeModuleResolutionKind::Node10,
                    Vue3TypeModuleResolutionKind::Node16,
                    Vue3TypeModuleResolutionKind::NodeNext,
                    Vue3TypeModuleResolutionKind::Bundler,
                ] {
                    for resolution_mode in [
                        Vue3TypeResolutionMode::Import,
                        Vue3TypeResolutionMode::Require,
                    ] {
                        let resolver = Vue3TypeResolverContext {
                            typescript_version: (6, 0, 3).into(),
                            module_resolution,
                            ..Vue3TypeResolverContext::default()
                        };
                        let strict = package_type == Some("module")
                            && matches!(
                                module_resolution,
                                Vue3TypeModuleResolutionKind::Node16
                                    | Vue3TypeModuleResolutionKind::NodeNext
                            )
                            && resolution_mode == Vue3TypeResolutionMode::Import;
                        let expected = if strict {
                            explicit_path.clone()
                        } else {
                            permissive_path.clone()
                        }
                        .map_or(
                                Vue3PackageJsonTypeResolution::NoPackageTypeEntry,
                                Vue3PackageJsonTypeResolution::Resolved,
                            );
                        assert_eq!(
                            resolve_vue3_package_json_type_entry_with_mode(
                                &package_dir,
                                None,
                                resolution_mode,
                                &resolver,
                            ),
                            expected,
                            "{field} {package_type_name} {target_kind} {module_resolution:?} {resolution_mode:?}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn vue3_package_root_fields_separate_type_and_javascript_passes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cases = vec![
        (
            "types-raw",
            serde_json::json!({"types":"types.js"}),
            vec![("types.js", "export const implementation = true;")],
            None,
            None,
            None,
        ),
        (
            "types-raw-main",
            serde_json::json!({"types":"types.js","main":"main.js"}),
            vec![
                ("types.js", "export const ignoredImplementation = true;"),
                ("main.js", "export const implementation = true;"),
            ],
            Some("main.js"),
            Some("main.js"),
            None,
        ),
        (
            "types-declaration-main",
            serde_json::json!({"types":"types.js","main":"main.js"}),
            vec![
                ("types.d.ts", "export interface DeclaredProps {}"),
                ("main.js", "export const implementation = true;"),
            ],
            Some("types.d.ts"),
            Some("types.d.ts"),
            Some("types.d.ts"),
        ),
        (
            "types-typescript-to-declaration",
            serde_json::json!({"types":"entry.ts"}),
            vec![("entry.d.ts", "export interface DeclaredProps {}")],
            Some("entry.d.ts"),
            Some("entry.d.ts"),
            Some("entry.d.ts"),
        ),
        (
            "typings-shadow",
            serde_json::json!({
                "typings":"typings.js",
                "types":"types.d.ts",
                "main":"main.js"
            }),
            vec![
                ("typings.js", "export const ignoredImplementation = true;"),
                ("types.d.ts", "export interface IgnoredTypesProps {}"),
                ("main.js", "export const implementation = true;"),
            ],
            Some("main.js"),
            Some("main.js"),
            None,
        ),
        (
            "main-extensionless",
            serde_json::json!({"main":"entry"}),
            vec![("entry.js", "export const implementation = true;")],
            Some("entry.js"),
            None,
            None,
        ),
        (
            "main-directory",
            serde_json::json!({"main":"directory"}),
            vec![
                ("directory/package.json", r#"{"types":"wrong.d.ts"}"#),
                (
                    "directory/wrong.d.ts",
                    "export interface WrongNestedManifestProps {}",
                ),
                (
                    "directory/index.js",
                    "export const implementation = true;",
                ),
            ],
            Some("directory/index.js"),
            None,
            None,
        ),
        (
            "types-directory-javascript",
            serde_json::json!({"types":"directory"}),
            vec![(
                "directory/index.js",
                "export const ignoredImplementation = true;",
            )],
            None,
            None,
            None,
        ),
        (
            "main-prefers-declaration",
            serde_json::json!({"main":"entry.js"}),
            vec![
                ("entry.d.ts", "export interface DeclaredProps {}"),
                ("entry.js", "export const implementation = true;"),
            ],
            Some("entry.d.ts"),
            Some("entry.d.ts"),
            Some("entry.d.ts"),
        ),
        (
            "main-typescript-to-javascript",
            serde_json::json!({"main":"entry.ts"}),
            vec![("entry.js", "export const implementation = true;")],
            Some("entry.js"),
            Some("entry.js"),
            None,
        ),
        (
            "main-typescript-to-declaration",
            serde_json::json!({"main":"entry.ts"}),
            vec![("entry.d.ts", "export interface DeclaredProps {}")],
            Some("entry.d.ts"),
            Some("entry.d.ts"),
            Some("entry.d.ts"),
        ),
        (
            "main-appended-javascript",
            serde_json::json!({"main":"entry.css"}),
            vec![("entry.css.js", "export const implementation = true;")],
            Some("entry.css.js"),
            None,
            None,
        ),
    ];

    for (case, base_manifest, files, permissive, strict, reference) in cases {
        for (package_type_name, package_type) in [
            ("unspecified", None),
            ("commonjs", Some("commonjs")),
            ("module", Some("module")),
        ] {
            let package_dir = dir.path().join(format!("{case}-{package_type_name}"));
            std::fs::create_dir_all(&package_dir).expect("create two-phase package");
            for (relative, source) in files.iter() {
                let path = package_dir.join(relative);
                std::fs::create_dir_all(path.parent().expect("fixture parent"))
                    .expect("create two-phase fixture parent");
                std::fs::write(path, source).expect("write two-phase fixture");
            }
            let mut manifest = base_manifest
                .as_object()
                .expect("object package manifest")
                .clone();
            if let Some(package_type) = package_type {
                manifest.insert("type".to_string(), serde_json::json!(package_type));
            }
            std::fs::write(
                package_dir.join("package.json"),
                serde_json::Value::Object(manifest).to_string(),
            )
            .expect("write two-phase package manifest");

            for module_resolution in [
                Vue3TypeModuleResolutionKind::Node10,
                Vue3TypeModuleResolutionKind::Node16,
                Vue3TypeModuleResolutionKind::NodeNext,
                Vue3TypeModuleResolutionKind::Bundler,
            ] {
                for resolution_mode in [
                    Vue3TypeResolutionMode::Import,
                    Vue3TypeResolutionMode::Require,
                ] {
                    let resolver = Vue3TypeResolverContext {
                        typescript_version: (6, 0, 3).into(),
                        module_resolution,
                        ..Vue3TypeResolverContext::default()
                    };
                    let uses_strict_package_target = package_type == Some("module")
                        && resolution_mode == Vue3TypeResolutionMode::Import
                        && matches!(
                            module_resolution,
                            Vue3TypeModuleResolutionKind::Node16
                                | Vue3TypeModuleResolutionKind::NodeNext
                        );
                    let expected = if uses_strict_package_target {
                        strict
                    } else {
                        permissive
                    }
                    .map(|relative| package_dir.join(relative));
                    let actual = vue3_package_resolution_path(
                        resolve_vue3_package_json_type_entry_with_mode(
                            &package_dir,
                            None,
                            resolution_mode,
                            &resolver,
                        ),
                    );
                    assert_eq!(
                        actual, expected,
                        "{case} {package_type_name} {module_resolution:?} {resolution_mode:?}",
                    );
                    let reference_expected =
                        reference.map(|relative| package_dir.join(relative));
                    let reference_actual = vue3_package_resolution_path(
                        resolve_vue3_package_json_type_reference_entry(
                            &package_dir,
                            None,
                            Some(resolution_mode),
                            &resolver,
                        ),
                    );
                    assert_eq!(
                        reference_actual, reference_expected,
                        "type reference {case} {package_type_name} {module_resolution:?} {resolution_mode:?}",
                    );
                    assert_eq!(
                        resolver.external_type_session.stats().metadata_files_read,
                        1,
                        "nested manifests must remain unread for {case} {package_type_name} {module_resolution:?} {resolution_mode:?}",
                    );
                    assert!(!resolver.external_type_session.metadata_is_blocked());
                }
            }
        }
    }
}

#[test]
fn vue3_package_root_field_phase_transition_obeys_probe_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create two-phase budget package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"types.js","main":"main.js"}"#,
    )
    .expect("write two-phase budget manifest");
    std::fs::write(
        package_dir.join("types.js"),
        "export const ignoredImplementation = true;",
    )
    .expect("write ignored type implementation");
    let main = package_dir.join("main.js");
    std::fs::write(&main, "export const implementation = true;")
        .expect("write main implementation");

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 7,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &accepted),
        Vue3PackageJsonTypeResolution::Resolved(main)
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        7
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 6,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &rejected),
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        6
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_package_resolution_finishes_the_type_index_before_the_javascript_phase() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create phased package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"missing.js","main":"main.js"}"#,
    )
    .expect("write phased package manifest");
    let index = package_dir.join("index.d.ts");
    std::fs::write(&index, "export interface IndexProps {}")
        .expect("write phased type index");
    std::fs::write(
        package_dir.join("main.js"),
        "export const implementation = true;",
    )
    .expect("write phased JavaScript main");

    for module_resolution in [
        Vue3TypeModuleResolutionKind::Node10,
        Vue3TypeModuleResolutionKind::Node16,
        Vue3TypeModuleResolutionKind::NodeNext,
        Vue3TypeModuleResolutionKind::Bundler,
    ] {
        for resolution_mode in [
            Vue3TypeResolutionMode::Import,
            Vue3TypeResolutionMode::Require,
        ] {
            let resolver = Vue3TypeResolverContext {
                module_resolution,
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_package_type_entry_with_mode(
                    &package_dir,
                    None,
                    resolution_mode,
                    &resolver,
                ),
                Some(index.clone()),
                "{module_resolution:?} {resolution_mode:?}",
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
    }
}

#[test]
fn vue3_types_versions_reselect_the_root_source_for_each_resolution_phase() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create phased typesVersions package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{
            "types":"missing-types.d.ts",
            "main":"runtime",
            "typesVersions":{"*":{
                "missing-types.d.ts":["missing-target"],
                "runtime":["mapped-runtime"]
            }}
        }"#,
    )
    .expect("write phased typesVersions manifest");
    let runtime = package_dir.join("mapped-runtime.js");
    std::fs::write(&runtime, "export const runtime = true;")
        .expect("write mapped JavaScript runtime");

    let resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_type_entry_with_mode(
            &package_dir,
            None,
            Vue3TypeResolutionMode::Import,
            &resolver,
        ),
        Some(runtime),
    );
    assert_eq!(
        resolver
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        2,
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());

    let reference = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_reference_entry(
            &package_dir,
            None,
            None,
            &reference,
        ),
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry,
    );
    assert_eq!(
        reference
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        1,
    );
    assert!(!reference.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_types_versions_use_index_as_the_default_root_source() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create default-source package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"typesVersions":{"*":{"index":["mapped.d.ts"],"index.d.ts":["wrong.d.ts"]}}}"#,
    )
    .expect("write default-source manifest");
    let mapped = package_dir.join("mapped.d.ts");
    std::fs::write(&mapped, "export interface MappedProps {}")
        .expect("write default-source target");
    std::fs::write(
        package_dir.join("wrong.d.ts"),
        "export interface WrongProps {}",
    )
    .expect("write default-source decoy");

    assert_eq!(
        resolve_vue3_package_type_entry(&package_dir, None, &Vue3TypeResolverContext::default()),
        Some(mapped),
    );
}

#[test]
fn vue3_types_versions_cannot_redirect_unsafe_root_fields_back_into_the_package() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create unsafe-source package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"../outside.d.ts","typesVersions":{"*":{"../outside.d.ts":["inside.d.ts"]}}}"#,
    )
    .expect("write unsafe-source manifest");
    std::fs::write(
        package_dir.join("inside.d.ts"),
        "export interface IncorrectlyRedirectedProps {}",
    )
    .expect("write unsafe-source mapping target");

    assert_eq!(
        resolve_vue3_package_json_type_entry(
            &package_dir,
            None,
            &Vue3TypeResolverContext::default(),
        ),
        Vue3PackageJsonTypeResolution::Blocked,
    );
}

#[test]
fn vue3_types_versions_explicit_raw_targets_precede_phase_replacements() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create raw-priority package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"source.d.ts","typesVersions":{"*":{"source.d.ts":["mapped.js"]}}}"#,
    )
    .expect("write raw-priority manifest");
    let raw = package_dir.join("mapped.js");
    std::fs::write(&raw, "export const raw = true;").expect("write raw target");
    std::fs::write(
        package_dir.join("mapped.d.ts"),
        "export interface ReplacementProps {}",
    )
    .expect("write replacement decoy");

    assert_eq!(
        resolve_vue3_package_type_entry(&package_dir, None, &Vue3TypeResolverContext::default()),
        Some(raw),
    );
}

#[test]
fn vue3_types_versions_raw_priority_uses_the_unsubstituted_target_template() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    let mapped_dir = package_dir.join("mapped");
    std::fs::create_dir_all(&mapped_dir).expect("create template-priority package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"typesVersions":{"*":{"*":["mapped/*"]}}}"#,
    )
    .expect("write template-priority manifest");
    std::fs::write(
        mapped_dir.join("feature.js"),
        "export const implementation = true;",
    )
    .expect("write captured raw target");
    let declaration = mapped_dir.join("feature.d.ts");
    std::fs::write(&declaration, "export interface FeatureProps {}")
        .expect("write captured declaration target");

    assert_eq!(
        resolve_vue3_package_type_entry(
            &package_dir,
            Some("feature.js"),
            &Vue3TypeResolverContext::default(),
        ),
        Some(declaration),
    );
}

#[test]
fn vue3_types_versions_type_references_reject_raw_javascript_without_panicking() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create reference package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"source.d.ts","typesVersions":{"*":{"source.d.ts":["mapped.js"]}}}"#,
    )
    .expect("write reference manifest");
    std::fs::write(
        package_dir.join("mapped.js"),
        "export const implementation = true;",
    )
    .expect("write reference raw target");

    let resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_package_json_type_reference_entry(
            &package_dir,
            None,
            None,
            &resolver,
        ),
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry,
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_types_versions_javascript_targets_follow_strict_package_rules() {
    let dir = tempfile::tempdir().expect("temp dir");
    for target_kind in ["extensionless", "directory"] {
        let package_dir = dir.path().join(target_kind);
        std::fs::create_dir_all(&package_dir).expect("create strict JavaScript package");
        let target = match target_kind {
            "extensionless" => {
                let target = package_dir.join("mapped.js");
                std::fs::write(&target, "export const mapped = true;")
                    .expect("write extensionless JavaScript target");
                target
            }
            "directory" => {
                let directory = package_dir.join("mapped");
                std::fs::create_dir_all(&directory).expect("create JavaScript target directory");
                let target = directory.join("index.js");
                std::fs::write(&target, "export const mapped = true;")
                    .expect("write directory JavaScript target");
                target
            }
            _ => unreachable!(),
        };
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"type":"module","typesVersions":{"*":{"index":["mapped"]}}}"#,
        )
        .expect("write strict JavaScript manifest");

        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node10,
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
            Vue3TypeModuleResolutionKind::Bundler,
        ] {
            for resolution_mode in [
                Vue3TypeResolutionMode::Import,
                Vue3TypeResolutionMode::Require,
            ] {
                let resolver = Vue3TypeResolverContext {
                    typescript_version: (6, 0, 3).into(),
                    module_resolution,
                    ..Vue3TypeResolverContext::default()
                };
                let strict = resolution_mode == Vue3TypeResolutionMode::Import
                    && matches!(
                        module_resolution,
                        Vue3TypeModuleResolutionKind::Node16
                            | Vue3TypeModuleResolutionKind::NodeNext
                    );
                assert_eq!(
                    resolve_vue3_package_type_entry_with_mode(
                        &package_dir,
                        None,
                        resolution_mode,
                        &resolver,
                    ),
                    (!strict).then_some(target.clone()),
                    "{target_kind} {module_resolution:?} {resolution_mode:?}",
                );
                assert!(!resolver.external_type_session.metadata_is_blocked());
            }
        }
    }
}

#[test]
fn vue3_types_versions_matched_missing_targets_suppress_same_phase_fallbacks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create matched-missing package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"types":"types.d.ts","typesVersions":{"*":{"types.d.ts":["missing.d.ts"]}}}"#,
    )
    .expect("write matched-missing manifest");
    std::fs::write(
        package_dir.join("types.d.ts"),
        "export interface WrongFieldProps {}",
    )
    .expect("write blocked field fallback");
    std::fs::write(
        package_dir.join("index.d.ts"),
        "export interface WrongIndexProps {}",
    )
    .expect("write blocked index fallback");

    let resolver = Vue3TypeResolverContext::default();
    assert!(resolve_vue3_package_type_entry(&package_dir, None, &resolver).is_none());
    assert!(!resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_types_versions_javascript_phase_obeys_fanout_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    std::fs::create_dir_all(&package_dir).expect("create fanout package");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"typesVersions":{"*":{"index":["mapped"]}}}"#,
    )
    .expect("write fanout manifest");
    let target = package_dir.join("mapped.js");
    std::fs::write(&target, "export const mapped = true;")
        .expect("write fanout JavaScript target");

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_type_entry(&package_dir, None, &accepted),
        Some(target),
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        2,
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());
    let exact_probe_count = accepted
        .external_type_session
        .stats()
        .metadata_resolution_path_probes;

    let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        max_metadata_resolution_path_probes: exact_probe_count,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_type_entry(&package_dir, None, &exact),
        Some(package_dir.join("mapped.js")),
    );
    assert_eq!(
        exact
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        exact_probe_count,
    );
    assert!(!exact.external_type_session.metadata_is_blocked());

    let probe_limited = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        max_metadata_resolution_path_probes: exact_probe_count - 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(resolve_vue3_package_type_entry(&package_dir, None, &probe_limited).is_none());
    assert_eq!(
        probe_limited
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        exact_probe_count - 1,
    );
    assert!(probe_limited.external_type_session.metadata_is_blocked());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(resolve_vue3_package_type_entry(&package_dir, None, &rejected).is_none());
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        1,
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_types_versions_targets_follow_root_and_subpath_path_rules() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (scope, subpath, mapping_source) in [
        ("root", None, "source.d.ts"),
        ("subpath", Some("feature"), "feature"),
    ] {
        for (package_type_name, package_type) in [
            ("unspecified", None),
            ("commonjs", Some("commonjs")),
            ("module", Some("module")),
        ] {
            for target_kind in [
                "extensionless",
                "directory",
                "explicit",
                "appended",
                "arbitrary-declaration",
                "raw-javascript",
            ] {
                let package_dir = dir
                    .path()
                    .join(format!("{scope}-{package_type_name}-{target_kind}"));
                let fixture = write_vue3_package_target_fixture(&package_dir, target_kind);
                let mut mappings = serde_json::Map::new();
                mappings.insert(
                    mapping_source.to_string(),
                    serde_json::json!([fixture.target]),
                );
                let mut manifest = serde_json::Map::new();
                manifest.insert("types".to_string(), serde_json::json!("source.d.ts"));
                manifest.insert(
                    "typesVersions".to_string(),
                    serde_json::json!({"*": serde_json::Value::Object(mappings)}),
                );
                if let Some(package_type) = package_type {
                    manifest.insert("type".to_string(), serde_json::json!(package_type));
                }
                std::fs::write(
                    package_dir.join("package.json"),
                    serde_json::Value::Object(manifest).to_string(),
                )
                .expect("write typesVersions package manifest");

                for module_resolution in [
                    Vue3TypeModuleResolutionKind::Node10,
                    Vue3TypeModuleResolutionKind::Node16,
                    Vue3TypeModuleResolutionKind::NodeNext,
                    Vue3TypeModuleResolutionKind::Bundler,
                ] {
                    for resolution_mode in [
                        Vue3TypeResolutionMode::Import,
                        Vue3TypeResolutionMode::Require,
                    ] {
                        let resolver = Vue3TypeResolverContext {
                            typescript_version: (6, 0, 3).into(),
                            module_resolution,
                            ..Vue3TypeResolverContext::default()
                        };
                        let strict = resolution_mode == Vue3TypeResolutionMode::Import
                            && matches!(
                                module_resolution,
                                Vue3TypeModuleResolutionKind::Node16
                                    | Vue3TypeModuleResolutionKind::NodeNext
                            )
                            && (subpath.is_some() || package_type == Some("module"));
                        let expected = if strict {
                            fixture.explicit_path.clone()
                        } else {
                            fixture.permissive_path.clone()
                        };
                        let actual = vue3_package_resolution_path(
                            resolve_vue3_package_json_type_entry_with_mode(
                                &package_dir,
                                subpath,
                                resolution_mode,
                                &resolver,
                            ),
                        );
                        assert_eq!(
                            actual, expected,
                            "{scope} {package_type_name} {target_kind} {module_resolution:?} {resolution_mode:?}",
                        );
                        assert!(!resolver.external_type_session.metadata_is_blocked());
                    }
                }
            }
        }
    }
}

#[test]
fn vue3_node_esm_subpath_index_fallback_stops_at_typescript_5_8() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (exports_name, exports) in [
        ("absent", None),
        ("null", Some(serde_json::Value::Null)),
        ("false", Some(serde_json::json!(false))),
        ("zero", Some(serde_json::json!(0))),
        ("empty-string", Some(serde_json::json!(""))),
    ] {
        let package_dir = dir.path().join(exports_name);
        let mapped = package_dir.join("mapped.dir");
        let plain = package_dir.join("plain.dir");
        for directory in [&mapped, &plain] {
            std::fs::create_dir_all(directory).expect("create historical fallback directory");
            std::fs::write(
                directory.join("package.json"),
                r#"{"types":"wrong.d.ts"}"#,
            )
            .expect("write historical nested manifest decoy");
            std::fs::write(
                directory.join("wrong.d.ts"),
                "export interface WrongNestedManifestProps {}",
            )
            .expect("write historical nested manifest target");
            std::fs::write(
                directory.join("index.d.ts"),
                "export interface HistoricalIndexProps {}",
            )
            .expect("write historical index target");
        }
        let mut manifest = serde_json::Map::new();
        manifest.insert(
            "typesVersions".to_string(),
            serde_json::json!({"*": {"mapped": ["mapped.dir"]}}),
        );
        if let Some(exports) = exports {
            manifest.insert("exports".to_string(), exports);
        }
        std::fs::write(
            package_dir.join("package.json"),
            serde_json::Value::Object(manifest).to_string(),
        )
        .expect("write historical fallback package manifest");

        for (typescript_version, before_5_8) in [((5, 7, 3), true), ((5, 8, 3), false)] {
            for module_resolution in [
                Vue3TypeModuleResolutionKind::Node16,
                Vue3TypeModuleResolutionKind::NodeNext,
            ] {
                let resolver = Vue3TypeResolverContext {
                    typescript_version: typescript_version.into(),
                    module_resolution,
                    ..Vue3TypeResolverContext::default()
                };
                let allows_fallback = before_5_8 && matches!(exports_name, "absent" | "null");
                for (subpath, expected_path) in [
                    ("mapped", mapped.join("index.d.ts")),
                    ("plain.dir", plain.join("index.d.ts")),
                ] {
                    let actual = vue3_package_resolution_path(
                        resolve_vue3_package_json_type_entry_with_mode(
                            &package_dir,
                            Some(subpath),
                            Vue3TypeResolutionMode::Import,
                            &resolver,
                        ),
                    );
                    assert_eq!(
                        actual,
                        allows_fallback.then_some(expected_path),
                        "{exports_name} {typescript_version:?} {module_resolution:?} {subpath}",
                    );
                }
                assert_eq!(
                    resolver.external_type_session.stats().metadata_files_read,
                    1,
                    "nested manifests must remain unread for {exports_name} {typescript_version:?} {module_resolution:?}",
                );
                assert!(!resolver.external_type_session.metadata_is_blocked());
            }
        }
    }
}

#[test]
fn vue3_types_versions_legacy_index_fallback_obeys_probe_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    let mapped = package_dir.join("mapped");
    std::fs::create_dir_all(&mapped).expect("create mapped fallback directory");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"typesVersions":{"*":{"feature":["mapped"]}}}"#,
    )
    .expect("write fallback budget package manifest");
    let target = mapped.join("index.d.ts");
    std::fs::write(&target, "export interface BudgetedFallbackProps {}")
        .expect("write fallback budget target");

    let accepted = Vue3TypeResolverContext {
        typescript_version: (5, 7, 3).into(),
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
        resolve_vue3_package_json_type_entry_with_mode(
            &package_dir,
            Some("feature"),
            Vue3TypeResolutionMode::Import,
            &accepted,
        ),
        Vue3PackageJsonTypeResolution::Resolved(target)
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        3
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        1
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let rejected = Vue3TypeResolverContext {
        typescript_version: (5, 7, 3).into(),
        module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
        external_type_session: Vue3ExternalTypeLoadSession::with_limits(
            Vue3ExternalTypeLoadLimits {
                max_metadata_resolution_path_probes: 2,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ),
        ..Vue3TypeResolverContext::default()
    };
    assert_eq!(
        resolve_vue3_package_json_type_entry_with_mode(
            &package_dir,
            Some("feature"),
            Vue3TypeResolutionMode::Import,
            &rejected,
        ),
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        2
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_bare_package_subpaths_cannot_escape_package_root() {
    assert_eq!(
        vue3_package_import_parts("package/feature/item"),
        Some(("package".into(), Some("feature/item".into())))
    );
    assert_eq!(
        vue3_package_import_parts("@scope/package/feature/item"),
        Some((
            "@scope/package".into(),
            Some("feature/item".into())
        ))
    );
    for source in [
        "package/",
        "package//item",
        "package/./item",
        "package/../item",
        "package/item/..",
        "package\\..\\item",
        "@scope/..",
        "@scope/package/",
        "@scope/package/../../item",
    ] {
        assert!(
            vue3_package_import_parts(source).is_none(),
            "unsafe package specifier was accepted: {source}"
        );
    }

    let dir = tempfile::tempdir().expect("temp dir");
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir_all(node_modules.join("package")).expect("create package");
    std::fs::create_dir_all(node_modules.join("@scope").join("package"))
        .expect("create scoped package");
    let outside = dir.path().join("outside.d.ts");
    std::fs::write(&outside, "export interface Escaped { value: string }")
        .expect("write escaped target");
    let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();

    for source in [
        "package/../../outside.d.ts",
        "@scope/package/../../../outside.d.ts",
    ] {
        assert!(
            resolve_vue3_bare_type_import(
                &filename,
                source,
                &Vue3TypeResolverContext::default(),
            )
            .is_none(),
            "package subpath escaped its package root: {source}"
        );
    }
}

#[test]
fn vue3_path_normalization_preserves_unresolved_parent_components() {
    for (input, expected) in [
        ("../x", "../x"),
        ("a/../../x", "../x"),
        ("../../a/../x", "../../x"),
        ("a/./b/../x", "a/x"),
    ] {
        let normalized = normalize_path_components(PathBuf::from(input));
        assert_eq!(normalized, PathBuf::from(expected), "{input}");
        assert_eq!(
            normalize_path_components(normalized.clone()),
            normalized,
            "normalization was not idempotent for {input}"
        );
    }

    let root = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
    assert_eq!(
        normalize_path_components(root.join("a").join("..").join("..").join("x")),
        root.join("x")
    );
    assert_ne!(
        normalize_path_components(PathBuf::from("../x")),
        normalize_path_components(PathBuf::from("x"))
    );

    let resolver = Vue3TypeResolverContext::default();
    assert_eq!(
        vue3_tsconfig_target_path(
            Path::new("config"),
            Path::new("config"),
            "../../shared/x",
            &resolver,
        ),
        Some(PathBuf::from("../shared/x"))
    );
    assert_eq!(
        vue3_node_modules_search_paths_from_dir(Path::new("../workspace/src"), &resolver).next(),
        Some(PathBuf::from("../workspace/src/node_modules"))
    );
}

#[test]
fn vue3_ancestor_search_is_lazy_and_depth_bounded() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_DEPTH, 128);
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_ANCESTOR_SEARCH_PATH_BYTES, 64 * 1024);
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_ancestor_search_depth: 2,
        max_ancestor_search_entries: 8,
        max_ancestor_search_weight: 1024,
        max_ancestor_search_path_bytes: 1024,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let mut paths =
        vue3_node_modules_search_paths_from_dir(Path::new("a/b/c"), &resolver);

    assert_eq!(paths.next(), Some(PathBuf::from("a/b/c/node_modules")));
    assert_eq!(resolver.external_type_session.stats().ancestor_search_entries, 1);
    assert!(!resolver.external_type_session.metadata_is_blocked());
    assert_eq!(paths.next(), Some(PathBuf::from("a/b/node_modules")));
    assert_eq!(paths.next(), None);
    assert!(resolver.external_type_session.metadata_is_blocked());
    assert_eq!(resolver.external_type_session.stats().ancestor_search_entries, 2);
}

#[test]
fn vue3_ancestor_search_budgets_unique_native_directories() {
    let start = Path::new("a/b");
    let baseline = Vue3TypeResolverContext::default();
    let ancestor_count =
        vue3_node_modules_search_paths_from_dir(start, &baseline).count();
    let baseline_stats = baseline.external_type_session.stats();
    let exact_weight = baseline_stats.ancestor_search_weight;
    assert_eq!(baseline_stats.ancestor_search_entries, ancestor_count);
    let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_ancestor_search_depth: ancestor_count,
        max_ancestor_search_entries: ancestor_count,
        max_ancestor_search_weight: exact_weight,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        vue3_node_modules_search_paths_from_dir(start, &exact).count(),
        ancestor_count
    );
    assert_eq!(
        vue3_node_modules_search_paths("a/b/Comp.vue", &exact).count(),
        ancestor_count
    );
    let _ = vue3_tsconfig_search_paths("a/b/Comp.vue", &exact).collect::<Vec<_>>();
    let stats = exact.external_type_session.stats();
    assert_eq!(stats.ancestor_search_entries, ancestor_count);
    assert_eq!(stats.ancestor_search_weight, exact_weight);
    assert!(!exact.external_type_session.metadata_is_blocked());

    assert_eq!(
        vue3_node_modules_search_paths_from_dir(Path::new("a/c"), &exact).next(),
        None
    );
    assert!(exact.external_type_session.metadata_is_blocked());
    let stats = exact.external_type_session.stats();
    assert_eq!(stats.ancestor_search_entries, ancestor_count);
    assert_eq!(stats.ancestor_search_weight, exact_weight);

    let weight_limited =
        vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_ancestor_search_depth: ancestor_count,
            max_ancestor_search_entries: ancestor_count,
            max_ancestor_search_weight: exact_weight - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert_eq!(
        vue3_node_modules_search_paths_from_dir(start, &weight_limited).count(),
        ancestor_count - 1
    );
    assert!(weight_limited
        .external_type_session
        .metadata_is_blocked());
    assert_eq!(
        weight_limited
            .external_type_session
            .stats()
            .ancestor_search_entries,
        ancestor_count - 1
    );

    let path_limited =
        vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_ancestor_search_path_bytes: 16,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert_eq!(
        vue3_node_modules_search_paths_from_dir(
            Path::new("this-directory-is-too-long"),
            &path_limited,
        )
        .next(),
        None
    );
    assert!(path_limited.external_type_session.metadata_is_blocked());
    assert_eq!(
        path_limited
            .external_type_session
            .stats()
            .ancestor_search_entries,
        0
    );
}

#[cfg(windows)]
#[test]
fn vue3_path_normalization_preserves_windows_root_semantics() {
    for (input, expected) in [
        (r"C:..\x", r"C:..\x"),
        (r"C:a\..\..\x", r"C:..\x"),
        (r"C:\a\..\..\x", r"C:\x"),
        (r"\a\..\..\x", r"\x"),
        (r"\\server\share\a\..\..\x", r"\\server\share\x"),
        (r"\\?\C:\a\..\..\x", r"\\?\C:\x"),
        (
            r"\\?\UNC\server\share\a\..\..\x",
            r"\\?\UNC\server\share\x",
        ),
    ] {
        assert_eq!(
            normalize_path_components(PathBuf::from(input)),
            PathBuf::from(expected),
            "{input}"
        );
    }
}

fn write_vue3_package_resolution_chain(root: &Path, count: usize) {
    assert!(count > 0);
    let mut package = root.to_path_buf();
    for index in 0..count {
        std::fs::create_dir_all(&package).expect("create nested type package");
        if index + 1 == count {
            std::fs::write(
                package.join("package.json"),
                r#"{"types":"index.d.ts"}"#,
            )
            .expect("write leaf package manifest");
            std::fs::write(
                package.join("index.d.ts"),
                "export interface NestedProps { value: string }",
            )
            .expect("write nested package type");
        } else {
            std::fs::write(package.join("package.json"), r#"{"types":"child"}"#)
                .expect("write nested package manifest");
            package = package.join("child");
        }
    }
}

#[test]
fn vue3_legacy_package_targets_do_not_recurse_into_nested_manifests() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_PACKAGE_RESOLUTION_DEPTH, 64);
    let dir = tempfile::tempdir().expect("temp dir");
    let accepted_package = dir.path().join("accepted");
    let rejected_package = dir.path().join("rejected");
    write_vue3_package_resolution_chain(&accepted_package, 2);
    write_vue3_package_resolution_chain(&rejected_package, 3);

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_package_resolution_depth: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&accepted_package, None, &accepted),
        Vue3PackageJsonTypeResolution::Resolved(accepted_package.join("child/index.d.ts"))
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_package_resolution_depth: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&rejected_package, None, &rejected),
        Vue3PackageJsonTypeResolution::NoPackageTypeEntry
    );
    assert!(!rejected.external_type_session.metadata_is_blocked());
}

#[cfg(unix)]
#[test]
fn vue3_package_metadata_bounds_aliases_and_preserves_non_utf8_identities() {
    use std::os::unix::ffi::OsStringExt;

    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    write_vue3_test_type_package(&package_dir, r#"{"types":"index.d.ts"}"#);
    let alias_dir = dir.path().join("alias");
    std::os::unix::fs::symlink(&package_dir, &alias_dir).expect("create package alias");
    let alias_limited = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_files: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(matches!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &alias_limited),
        Vue3PackageJsonTypeResolution::Resolved(_)
    ));
    assert_eq!(
        resolve_vue3_package_json_type_entry(&alias_dir, None, &alias_limited),
        Vue3PackageJsonTypeResolution::Blocked
    );

    let loop_resolver = Vue3TypeResolverContext::default();
    std::fs::write(package_dir.join("package.json"), r#"{"types":"loop"}"#)
        .expect("write cyclic package target");
    std::os::unix::fs::symlink(&package_dir, package_dir.join("loop"))
        .expect("create package target cycle");
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &loop_resolver),
        Vue3PackageJsonTypeResolution::Blocked
    );

    let first = dir
        .path()
        .join(std::ffi::OsString::from_vec(b"non-utf8-\xff".to_vec()));
    let second = dir
        .path()
        .join(std::ffi::OsString::from_vec(b"non-utf8-\xfe".to_vec()));
    write_vue3_test_type_package(&first, r#"{"types":"index.d.ts"}"#);
    write_vue3_test_type_package(&second, r#"{"types":"index.d.ts"}"#);
    let identity_resolver = Vue3TypeResolverContext::default();
    assert!(matches!(
        resolve_vue3_package_json_type_entry(&first, None, &identity_resolver),
        Vue3PackageJsonTypeResolution::Resolved(_)
    ));
    assert!(matches!(
        resolve_vue3_package_json_type_entry(&second, None, &identity_resolver),
        Vue3PackageJsonTypeResolution::Resolved(_)
    ));
    assert_eq!(
        identity_resolver
            .external_type_session
            .stats()
            .metadata_files_read,
        2
    );
}

fn write_vue3_tsconfig_chain(root: &Path, count: usize) -> Vec<PathBuf> {
    assert!(count > 0);
    let mut projects = Vec::new();
    for index in 0..count {
        let project = root.join(format!("project-{index}"));
        std::fs::create_dir_all(&project).expect("create tsconfig chain directory");
        let source = if index + 1 == count {
            r#"{"compilerOptions":{"paths":{"deep":["./types.ts"]}}}"#.to_string()
        } else {
            format!(r#"{{"extends":"../project-{}/tsconfig.json"}}"#, index + 1)
        };
        std::fs::write(project.join("tsconfig.json"), source).expect("write chained tsconfig");
        projects.push(project);
    }
    std::fs::write(
        projects.last().expect("leaf project").join("types.ts"),
        "export interface DeepProps { deep: string }",
    )
    .expect("write deep type");
    projects
}

#[test]
fn vue3_tsconfig_graph_checks_depth_before_warm_cache() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_DEPTH, 64);
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = write_vue3_tsconfig_chain(dir.path(), 3);
    let leaf_filename = projects[2].join("Comp.vue").to_string_lossy().to_string();
    let root_filename = projects[0].join("Comp.vue").to_string_lossy().to_string();
    let middle_filename = projects[1].join("Comp.vue").to_string_lossy().to_string();
    let target = projects[2].join("types.ts");

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_tsconfig_type_import(&middle_filename, "deep", &accepted),
        Some(target.clone())
    );

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_tsconfig_type_import(&leaf_filename, "deep", &rejected),
        Some(target)
    );
    assert!(resolve_vue3_tsconfig_type_import(&root_filename, "deep", &rejected).is_none());
    let stats = rejected.external_type_session.stats();
    assert_eq!(stats.metadata_files_read, 3);
    assert_eq!(stats.metadata_parse_cache_hits, 0);
    assert_eq!(stats.tsconfig_nodes, 3);

    let cycle_root = dir.path().join("cycle-root");
    let cycle_child = dir.path().join("cycle-child");
    std::fs::create_dir_all(&cycle_root).expect("create cycle root");
    std::fs::create_dir_all(&cycle_child).expect("create cycle child");
    std::fs::write(
        cycle_root.join("tsconfig.json"),
        r#"{"extends":"../cycle-child/tsconfig.json"}"#,
    )
    .expect("write cycle root tsconfig");
    std::fs::write(
        cycle_child.join("tsconfig.json"),
        r#"{
            "extends":"../cycle-root/tsconfig.json",
            "compilerOptions":{"paths":{"cycle-depth":["./types.ts"]}}
        }"#,
    )
    .expect("write cycle child tsconfig");
    let cycle_target = cycle_child.join("types.ts");
    std::fs::write(
        &cycle_target,
        "export interface CycleDepthProps { value: string }",
    )
    .expect("write cycle depth target");
    let cycle_filename = cycle_root.join("Comp.vue").to_string_lossy().to_string();
    let cycle_resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_tsconfig_type_import(
            &cycle_filename,
            "cycle-depth",
            &cycle_resolver,
        ),
        Some(cycle_target)
    );
}

#[test]
fn vue3_tsconfig_graph_ignores_seen_nodes_at_depth_boundary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("tsconfig.json");
    let shared = dir.path().join("shared.json");
    let branch = dir.path().join("branch.json");
    std::fs::write(
        &root,
        r#"{"extends":["./shared.json","./branch.json"]}"#,
    )
    .expect("write root tsconfig");
    std::fs::write(
        &shared,
        r#"{"compilerOptions":{"paths":{"diamond":["./types.ts"]}}}"#,
    )
    .expect("write shared tsconfig");
    std::fs::write(&branch, r#"{"extends":"./shared.json"}"#)
        .expect("write branch tsconfig");
    let target = dir.path().join("types.ts");
    std::fs::write(&target, "export interface DiamondProps { value: string }")
        .expect("write diamond type");
    let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        resolve_vue3_tsconfig_type_import(&filename, "diamond", &resolver),
        Some(target)
    );
    assert_eq!(resolver.external_type_session.stats().tsconfig_nodes, 3);
}

#[test]
fn vue3_tsconfig_graph_bounds_unique_nodes() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_TSCONFIG_NODES, 512);
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"extends":"./middle.json"}"#,
    )
    .expect("write root tsconfig");
    std::fs::write(
        dir.path().join("middle.json"),
        r#"{"extends":"./base.json"}"#,
    )
    .expect("write middle tsconfig");
    std::fs::write(
        dir.path().join("base.json"),
        r#"{"compilerOptions":{"paths":{"bounded":["./types.ts"]}}}"#,
    )
    .expect("write base tsconfig");
    let target = dir.path().join("types.ts");
    std::fs::write(&target, "export interface BoundedProps { value: string }")
        .expect("write bounded type");
    let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_nodes: 3,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_tsconfig_type_import(&filename, "bounded", &accepted),
        Some(target)
    );
    assert_eq!(accepted.external_type_session.stats().tsconfig_nodes, 3);

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_nodes: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(resolve_vue3_tsconfig_type_import(&filename, "bounded", &rejected).is_none());
    let stats = rejected.external_type_session.stats();
    assert_eq!(stats.tsconfig_nodes, 2);
    assert_eq!(stats.metadata_files_read, 2);
}

#[test]
fn vue3_tsconfig_discovery_budget_is_shared_across_extends_and_references() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source_dir = dir.path().join("src");
    let reference_dir = dir.path().join("reference");
    let type_package = reference_dir.join("types").join("package");
    std::fs::create_dir_all(&source_dir).expect("create source directory");
    std::fs::create_dir_all(&reference_dir).expect("create reference directory");
    write_vue3_test_type_package(&type_package, r#"{"types":"index.d.ts"}"#);
    let base_file = dir.path().join("base.d.ts");
    let root_file = dir.path().join("root.d.ts");
    std::fs::write(&base_file, "declare interface BaseGlobalProps {}")
        .expect("write base global type");
    std::fs::write(&root_file, "declare interface RootGlobalProps {}")
        .expect("write root global type");
    std::fs::write(
        dir.path().join("base.json"),
        r#"{"include":["./base.d.ts"],"compilerOptions":{"types":[]}}"#,
    )
    .expect("write base tsconfig");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{
            "extends":"./base.json",
            "files":["./root.d.ts"],
            "compilerOptions":{"types":[]},
            "references":[{"path":"./reference"}]
        }"#,
    )
    .expect("write root tsconfig");
    std::fs::write(
        reference_dir.join("tsconfig.json"),
        r#"{"files":[],"compilerOptions":{"typeRoots":["./types"]}}"#,
    )
    .expect("write referenced tsconfig");
    let filename = source_dir.join("Comp.vue").to_string_lossy().to_string();
    let package_file = type_package.join("index.d.ts");

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 4,
        max_tsconfig_discovery_files: 3,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_global_type_files(&filename, &accepted),
        vec![root_file, base_file, package_file]
    );
    let stats = accepted.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 4);
    assert_eq!(stats.tsconfig_discovery_files, 3);

    let entry_limited = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 3,
        max_tsconfig_discovery_files: 3,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(vue3_tsconfig_global_type_files(&filename, &entry_limited).is_empty());
    let stats = entry_limited.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 3);
    assert_eq!(stats.tsconfig_discovery_files, 2);
    assert_eq!(stats.metadata_files_read, 3);

    let file_limited = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 4,
        max_tsconfig_discovery_files: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(vue3_tsconfig_global_type_files(&filename, &file_limited).is_empty());
    let stats = file_limited.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 4);
    assert_eq!(stats.tsconfig_discovery_files, 2);
}

#[test]
fn vue3_tsconfig_type_root_enumeration_is_bounded_before_sorting() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source_dir = dir.path().join("src");
    let type_root = dir.path().join("types");
    let alpha = type_root.join("alpha");
    let zeta = type_root.join("zeta");
    std::fs::create_dir_all(&source_dir).expect("create source directory");
    write_vue3_test_type_package(&alpha, r#"{"types":"index.d.ts"}"#);
    write_vue3_test_type_package(&zeta, r#"{"types":"index.d.ts"}"#);
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"files":[],"compilerOptions":{"typeRoots":["./types"]}}"#,
    )
    .expect("write tsconfig");
    let filename = source_dir.join("Comp.vue").to_string_lossy().to_string();

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 3,
        max_tsconfig_discovery_files: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_global_type_files(&filename, &accepted),
        vec![alpha.join("index.d.ts"), zeta.join("index.d.ts")]
    );
    let stats = accepted.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 3);
    assert_eq!(stats.tsconfig_discovery_files, 2);

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 2,
        max_tsconfig_discovery_files: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(vue3_tsconfig_global_type_files(&filename, &rejected).is_empty());
    let stats = rejected.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 2);
    assert_eq!(stats.tsconfig_discovery_files, 0);
    assert_eq!(stats.metadata_files_read, 1);
}

#[test]
fn vue3_tsconfig_empty_types_skips_type_root_discovery() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("src")).expect("create source directory");
    let alias = dir.path().join("alias.ts");
    std::fs::write(&alias, "export interface AliasProps {}").expect("write alias type");
    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"files":[],"compilerOptions":{"types":[],"paths":{"alias":["./alias.ts"]}}}"#,
    )
    .expect("write tsconfig");
    let filename = dir
        .path()
        .join("src")
        .join("Comp.vue")
        .to_string_lossy()
        .to_string();
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 0,
        max_tsconfig_discovery_files: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert!(vue3_tsconfig_global_type_files(&filename, &resolver).is_empty());
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 0);
    assert_eq!(stats.tsconfig_discovery_files, 0);
    assert_eq!(
        resolve_vue3_tsconfig_type_import(&filename, "alias", &resolver),
        Some(alias)
    );
}

#[test]
fn vue3_tsconfig_named_type_candidates_are_charged_before_package_resolution() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first_root = dir.path().join("first");
    let second_root = dir.path().join("second");
    std::fs::create_dir_all(first_root.join("missing")).expect("create first candidate");
    std::fs::create_dir_all(second_root.join("missing")).expect("create second candidate");
    std::fs::write(second_root.join("missing").join("package.json"), "{")
        .expect("write malformed second manifest");
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_tsconfig_discovery_entries: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert!(vue3_tsconfig_named_type_global_type_files(
        &[first_root, second_root],
        "missing",
        &resolver,
    )
    .is_empty());
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.tsconfig_discovery_entries, 1);
    assert_eq!(stats.metadata_files_read, 1);
}

#[cfg(unix)]
#[test]
fn vue3_tsconfig_graph_uses_canonical_symlink_identity() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join("tsconfig.json");
    let alias = dir.path().join("alias.json");
    std::fs::write(
        &config,
        r#"{
            "extends": "./alias.json",
            "compilerOptions": { "paths": { "cycle": ["./types.ts"] } }
        }"#,
    )
    .expect("write cyclic tsconfig");
    std::os::unix::fs::symlink(&config, &alias).expect("create tsconfig symlink cycle");
    let target = dir.path().join("types.ts");
    std::fs::write(&target, "export interface CycleProps { value: string }")
        .expect("write cycle type");
    let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
    let resolver = Vue3TypeResolverContext::default();

    assert_eq!(
        resolve_vue3_tsconfig_type_import(&filename, "cycle", &resolver),
        Some(target)
    );
    let stats = resolver.external_type_session.stats();
    assert_eq!(stats.tsconfig_nodes, 1);
    assert_eq!(stats.metadata_files_read, 1);
}

fn write_vue3_metadata_budget_project(root: &Path, targets: &[&str]) -> (String, PathBuf) {
    let target = root.join("hit.ts");
    std::fs::write(&target, "export interface BudgetProps { value: string }")
        .expect("write metadata budget target");
    std::fs::write(
        root.join("tsconfig.json"),
        serde_json::json!({
            "compilerOptions": {
                "paths": { "budget": targets }
            }
        })
        .to_string(),
    )
    .expect("write metadata budget tsconfig");
    (
        root.join("Comp.vue").to_string_lossy().to_string(),
        target,
    )
}

#[test]
fn vue3_metadata_fanout_entries_are_bounded_and_cached() {
    assert_eq!(
        VUE3_EXTERNAL_TYPE_MAX_METADATA_FANOUT_ENTRIES,
        65_536
    );
    let accepted_dir = tempfile::tempdir().expect("accepted temp dir");
    let (accepted_filename, accepted_target) =
        write_vue3_metadata_budget_project(accepted_dir.path(), &["missing.ts", "hit.ts"]);
    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        resolve_vue3_type_import(&accepted_filename, "budget", &accepted),
        Some(accepted_target.clone())
    );
    let first_stats = accepted.external_type_session.stats();
    assert_eq!(first_stats.metadata_fanout_entries, 2);
    assert_eq!(
        resolve_vue3_type_import(&accepted_filename, "budget", &accepted),
        Some(accepted_target)
    );
    let cached_stats = accepted.external_type_session.stats();
    assert_eq!(
        cached_stats.metadata_fanout_entries,
        first_stats.metadata_fanout_entries
    );
    assert_eq!(
        cached_stats.metadata_resolution_path_probes,
        first_stats.metadata_resolution_path_probes
    );
    assert_eq!(cached_stats.resolution_cache_hits, 1);
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let zero = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(resolve_vue3_type_import(&accepted_filename, "budget", &zero).is_none());
    assert_eq!(
        zero.external_type_session.stats().metadata_fanout_entries,
        0
    );
    assert!(zero.external_type_session.metadata_is_blocked());

    let rejected_dir = tempfile::tempdir().expect("rejected temp dir");
    let (rejected_filename, _) = write_vue3_metadata_budget_project(
        rejected_dir.path(),
        &["missing.ts", "missing.ts", "hit.ts"],
    );
    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert!(resolve_vue3_type_import(&rejected_filename, "budget", &rejected).is_none());
    let stats = rejected.external_type_session.stats();
    assert_eq!(stats.metadata_fanout_entries, 2);
    assert_eq!(stats.resolution_cache_hits, 0);
    assert!(rejected.external_type_session.metadata_is_blocked());
    assert!(resolve_vue3_type_import(&rejected_filename, "budget", &rejected).is_none());
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .resolution_cache_hits,
        0
    );
}

#[test]
fn vue3_metadata_resolution_path_probes_are_bounded_before_success() {
    assert_eq!(
        VUE3_EXTERNAL_TYPE_MAX_METADATA_RESOLUTION_PATH_PROBES,
        131_072
    );
    let accepted_dir = tempfile::tempdir().expect("accepted temp dir");
    let candidate = accepted_dir.path().join("base");
    std::fs::create_dir_all(&candidate).expect("create accepted config directory");
    let accepted_target = candidate.join("tsconfig.json");
    std::fs::write(&accepted_target, "{}").expect("write accepted config");
    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        resolve_vue3_tsconfig_candidate_path(&candidate, false, &accepted),
        Some(accepted_target)
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        2
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let zero = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(resolve_vue3_tsconfig_candidate_path(&candidate, false, &zero).is_none());
    assert_eq!(
        zero.external_type_session
            .stats()
            .metadata_resolution_path_probes,
        0
    );
    assert!(zero.external_type_session.metadata_is_blocked());

    let rejected_dir = tempfile::tempdir().expect("rejected temp dir");
    let rejected_candidate = rejected_dir.path().join("base");
    std::fs::create_dir_all(&rejected_candidate).expect("create rejected config directory");
    std::fs::write(rejected_candidate.join("tsconfig.json"), "{}")
        .expect("write rejected config");
    let safe_relative = rejected_dir.path().join("safe.ts");
    std::fs::write(&safe_relative, "export interface SafeProps {}")
        .expect("write safe relative target");
    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert!(resolve_vue3_tsconfig_candidate_path(&rejected_candidate, false, &rejected).is_none());
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        1
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
    let importer = rejected_dir
        .path()
        .join("Comp.vue")
        .to_string_lossy()
        .to_string();
    assert_eq!(
        resolve_vue3_type_import(&importer, "./safe.ts", &rejected),
        Some(safe_relative)
    );
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        1
    );
}

#[test]
fn vue3_base_url_resolution_probes_are_exact_and_cached() {
    let dir = tempfile::tempdir().expect("temp dir");
    let base_url = dir.path().join("src");
    std::fs::create_dir_all(&base_url).expect("create baseUrl directory");
    let target = base_url.join("choice.ts");
    std::fs::write(&target, "export interface ChoiceProps { value: string }")
        .expect("write baseUrl target");

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_tsconfig_base_url_with_mode(
            &base_url,
            "choice",
            Vue3TypeResolutionMode::Import,
            &accepted,
        ),
        Some(target.clone())
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        1
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());
    assert!(resolve_vue3_tsconfig_base_url_with_mode(
        &base_url,
        &normalize_path_string(&target),
        Vue3TypeResolutionMode::Import,
        &accepted,
    )
    .is_none());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_resolution_path_probes: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(resolve_vue3_tsconfig_base_url_with_mode(
        &base_url,
        "choice",
        Vue3TypeResolutionMode::Import,
        &rejected,
    )
    .is_none());
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_resolution_path_probes,
        0
    );
    assert!(rejected.external_type_session.metadata_is_blocked());

    std::fs::write(
        dir.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"./src"}}"#,
    )
    .expect("write cached baseUrl config");
    let importer = dir.path().join("Comp.vue").to_string_lossy().to_string();
    let cached = Vue3TypeResolverContext::default();
    assert_eq!(
        resolve_vue3_type_import(&importer, "choice", &cached),
        Some(target.clone())
    );
    let first_stats = cached.external_type_session.stats();
    assert_eq!(
        resolve_vue3_type_import(&importer, "choice", &cached),
        Some(target)
    );
    let cached_stats = cached.external_type_session.stats();
    assert_eq!(
        cached_stats.metadata_files_read,
        first_stats.metadata_files_read
    );
    assert_eq!(
        cached_stats.metadata_resolution_path_probes,
        first_stats.metadata_resolution_path_probes
    );
    assert_eq!(cached_stats.resolution_cache_hits, 1);
    assert!(!cached.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_tsconfig_extends_and_references_share_fanout_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let base = dir.path().join("base.json");
    let referenced = dir.path().join("referenced.json");
    std::fs::write(&base, "{}").expect("write base config");
    std::fs::write(&referenced, "{}").expect("write referenced config");
    let value = serde_json::json!({
        "extends": ["./missing.json", "./base.json"],
        "references": [
            {"path": "./missing-reference.json"},
            {"path": "./referenced.json"}
        ]
    });
    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 4,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        vue3_tsconfig_extends_paths(&value, dir.path(), &accepted),
        vec![base]
    );
    assert_eq!(
        vue3_tsconfig_reference_paths(&value, dir.path(), &accepted),
        vec![referenced]
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        4
    );
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 3,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        vue3_tsconfig_extends_paths(&value, dir.path(), &rejected),
        vec![dir.path().join("base.json")]
    );
    assert!(vue3_tsconfig_reference_paths(&value, dir.path(), &rejected).is_empty());
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        3
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_types_versions_repeated_targets_consume_fanout_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package_dir = dir.path().join("package");
    write_vue3_test_type_package(
        &package_dir,
        r#"{
            "types":"index.d.ts",
            "typesVersions":{"*":{"index.d.ts":["missing.d.ts","hit.d.ts"]}}
        }"#,
    );
    let hit = package_dir.join("hit.d.ts");
    std::fs::write(&hit, "export interface VersionedProps {}")
        .expect("write versioned target");
    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });

    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &accepted),
        Vue3PackageJsonTypeResolution::Resolved(hit)
    );
    assert_eq!(
        accepted
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        2
    );

    std::fs::write(
        package_dir.join("package.json"),
        r#"{
            "types":"index.d.ts",
            "typesVersions":{"*":{"index.d.ts":["missing.d.ts","missing.d.ts","hit.d.ts"]}}
        }"#,
    )
    .expect("write repeated version targets");
    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&package_dir, None, &rejected),
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert_eq!(
        rejected
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        2
    );
    assert!(rejected.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_metadata_fanout_semantic_miss_does_not_block() {
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_metadata_fanout_entries: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let mappings = vue3_tsconfig_direct_path_mappings(
        &serde_json::json!({
            "compilerOptions": { "paths": { "missing": ["missing.ts"] } }
        }),
        Path::new("."),
        Path::new("."),
        &resolver,
    );

    assert!(resolve_vue3_tsconfig_path_mappings(&mappings, "missing", &resolver).is_none());
    assert_eq!(
        resolver
            .external_type_session
            .stats()
            .metadata_fanout_entries,
        1
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_package_condition_scanning_and_fallback_are_fanout_bounded() {
    let conditions = serde_json::json!({
        "unknown": "./inactive.d.ts",
        "types": "./valid.d.ts"
    });
    let invalid = serde_json::json!({
        "unknown": "./inactive.d.ts",
        "types": "./valid.d.ts",
        "0": "./invalid.d.ts"
    });
    let rejected_then_valid = serde_json::json!({
        "types": null,
        "default": "./valid.d.ts"
    });
    let nested_rejected_then_valid = serde_json::json!({
        "types": null,
        "default": { "types": "./valid.d.ts" }
    });
    for (target, limit, expected, blocked) in [
        (&conditions, 2, Some("./valid.d.ts"), false),
        (&conditions, 1, None, true),
        (&invalid, 3, Some("./valid.d.ts"), false),
        (&invalid, 2, None, true),
        (&rejected_then_valid, 2, Some("./valid.d.ts"), false),
        (&rejected_then_valid, 1, None, true),
        (&nested_rejected_then_valid, 3, Some("./valid.d.ts"), false),
        (&nested_rejected_then_valid, 2, None, true),
    ] {
        let resolver =
            vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
                max_metadata_fanout_entries: limit,
                ..Vue3ExternalTypeLoadLimits::default()
            });
        assert_eq!(
            vue3_package_exports_type_target(target, None, &resolver).as_deref(),
            expected
        );
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .metadata_fanout_entries,
            limit
        );
        assert_eq!(
            resolver.external_type_session.metadata_is_blocked(),
            blocked
        );
    }

    let blocked_then_valid = serde_json::json!({
        "types": "./target-that-exceeds-the-limit.d.ts",
        "default": "./ok.d.ts"
    });
    let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_generated_path_bytes: "./ok.d.ts".len(),
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(vue3_package_exports_type_target(&blocked_then_valid, None, &resolver).is_none());
    assert!(resolver.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_package_self_references_honor_metadata_and_source_budgets() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package = dir.path().join("node_modules").join("vuec-budget-self");
    std::fs::create_dir_all(&package).expect("create budget self package");
    std::fs::write(
        package.join("package.json"),
        r#"{
            "name":"vuec-budget-self",
            "exports":{"./leaf":{"types":"./leaf.d.mts"}}
        }"#,
    )
    .expect("write budget self manifest");
    let bridge = package.join("bridge.d.mts");
    let leaf = package.join("leaf.d.mts");
    std::fs::write(
        &bridge,
        "export { Leaf } from 'vuec-budget-self/leaf'",
    )
    .expect("write budget self bridge");
    std::fs::write(&leaf, "export interface Leaf { value: string }")
        .expect("write budget self leaf");

    let load = |resolver: &Vue3TypeResolverContext| {
        vue3_external_type_context_from_path(&bridge, &mut BTreeSet::new(), resolver)
            .expect("load bounded self-reference bridge")
    };
    let accepted =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_import_files: 2,
        max_metadata_fanout_entries: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let context = load(&accepted);
    assert!(context.declared_types.contains_key("Leaf"));
    let stats = accepted.external_type_session.stats();
    assert_eq!(stats.import_files_read, 2);
    assert_eq!(stats.metadata_files_read, 1);
    assert_eq!(stats.metadata_fanout_entries, 1);
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let no_metadata =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_import_files: 2,
        max_metadata_files: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let context = load(&no_metadata);
    assert!(!context.declared_types.contains_key("Leaf"));
    assert_eq!(no_metadata.external_type_session.stats().import_files_read, 1);
    assert!(no_metadata.external_type_session.metadata_is_blocked());

    let no_fanout =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_import_files: 2,
        max_metadata_fanout_entries: 0,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let context = load(&no_fanout);
    assert!(!context.declared_types.contains_key("Leaf"));
    let stats = no_fanout.external_type_session.stats();
    assert_eq!(stats.import_files_read, 1);
    assert_eq!(stats.metadata_fanout_entries, 0);
    assert!(no_fanout.external_type_session.metadata_is_blocked());

    let source_limited =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_import_files: 1,
        max_metadata_fanout_entries: 1,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    let context = load(&source_limited);
    assert!(!context.declared_types.contains_key("Leaf"));
    let stats = source_limited.external_type_session.stats();
    assert_eq!(stats.import_files_read, 1);
    assert_eq!(stats.metadata_fanout_entries, 1);
    assert!(!source_limited.external_type_session.metadata_is_blocked());
}

#[test]
fn vue3_dependency_package_imports_fail_closed_at_the_nearest_scope() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package = dir.path().join("node_modules").join("vuec-imports-boundary");
    std::fs::create_dir_all(&package).expect("create imports boundary package");
    let importer = package.join("index.d.mts");
    let target = package.join("ok.d.mts");
    std::fs::write(&importer, "export {};").expect("write imports boundary importer");
    std::fs::write(&target, "export interface Ok { value: string }")
        .expect("write imports boundary target");
    let rejected_manifests = [
        serde_json::json!({}),
        serde_json::json!({ "imports": null }),
        serde_json::json!({ "imports": [] }),
        serde_json::json!({ "imports": { "#other": "./ok.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": null } }),
        serde_json::json!({ "imports": { "#alias": [] } }),
        serde_json::json!({ "imports": { "#alias": [null] } }),
        serde_json::json!({ "imports": { "#alias": true } }),
        serde_json::json!({ "imports": { "#alias": 1 } }),
        serde_json::json!({ "imports": { "#alias": "../outside.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": "/outside.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": "C:/outside.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": "file:./ok.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": "./node_modules/ok.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": "./%2e%2e/ok.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": "./types%2fok.d.mts" } }),
        serde_json::json!({ "imports": { "#alias": { "unknown": "./ok.d.mts" } } }),
    ];
    for manifest in rejected_manifests {
        std::fs::write(package.join("package.json"), manifest.to_string())
            .expect("write rejected imports manifest");
        let resolver = vue3_node_next_type_resolver();
        assert!(resolve_vue3_type_import(
            &importer.to_string_lossy(),
            "#alias",
            &resolver,
        )
        .is_none());
        assert!(
            !resolver.external_type_session.metadata_is_blocked(),
            "semantic rejection blocked metadata for {manifest}"
        );
    }

    for manifest in [
        serde_json::json!({
            "imports": { "#alias": { "types": "./ok.d.mts", "0": "./missing.d.mts" } }
        }),
        serde_json::json!({
            "imports": { "#alias": { "types": null, "default": "./ok.d.mts" } }
        }),
        serde_json::json!({
            "imports": { "#alias": { "types": [], "default": "./ok.d.mts" } }
        }),
        serde_json::json!({
            "imports": { "#alias": { "types": [null, []], "default": "./ok.d.mts" } }
        }),
        serde_json::json!({
            "imports": {
                "#alias": [
                    { "node": null, "default": "./ok.d.mts" },
                    "./missing.d.mts"
                ]
            }
        }),
    ] {
        std::fs::write(package.join("package.json"), manifest.to_string())
            .expect("write conditional fallback imports manifest");
        let resolver = vue3_node_next_type_resolver();
        assert_eq!(
            resolve_vue3_type_import(&importer.to_string_lossy(), "#alias", &resolver),
            Some(target.clone()),
            "{manifest}"
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    std::fs::write(
        package.join("package.json"),
        r##"{"imports":{"#alias":[null,"../outside.d.mts","./ok.d.mts"]}}"##,
    )
    .expect("write array fallback imports manifest");
    let resolver = vue3_node_next_type_resolver();
    assert_eq!(
        resolve_vue3_type_import(&importer.to_string_lossy(), "#alias", &resolver),
        Some(target.clone())
    );
    assert!(!resolver.external_type_session.metadata_is_blocked());

    let paths_target = package.join("paths.d.mts");
    std::fs::write(
        &paths_target,
        "export interface PathsTarget { value: boolean }",
    )
    .expect("write nested imports paths target");
    std::fs::write(
        package.join("tsconfig.json"),
        r##"{"compilerOptions":{"baseUrl":".","paths":{"#target":["./paths.d.mts"]}}}"##,
    )
    .expect("write nested imports paths config");
    std::fs::write(
        package.join("package.json"),
        r##"{
            "imports":{
                "#alias":"#target",
                "#target":"./ok.d.mts",
                "#fallback":["#missing","./ok.d.mts"]
            }
        }"##,
    )
    .expect("write nested imports manifest");
    let resolver = vue3_node_next_type_resolver();
    assert_eq!(
        resolve_vue3_type_import(&importer.to_string_lossy(), "#alias", &resolver),
        Some(paths_target)
    );
    assert_eq!(
        resolve_vue3_type_import(&importer.to_string_lossy(), "#fallback", &resolver),
        Some(target.clone())
    );

    std::fs::write(
        package.join("package.json"),
        r##"{
            "imports":{
                "#feature/exact":"./missing.d.mts",
                "#feature/internal/*":"./specific.d.mts",
                "#feature/private/*":"./missing.d.mts",
                "#feature/*.js":"./javascript.d.mts",
                "#feature/*":"./broad.d.mts",
                "#prefix/":"./legacy/",
                "#legacy/":"./legacy/",
                "#legacy/*":"./legacy-pattern.d.mts",
                "#bad-prefix/":"./ok.d.mts"
            }
        }"##,
    )
    .expect("write imports selection manifest");
    let specific = package.join("specific.d.mts");
    let javascript = package.join("javascript.d.mts");
    let broad = package.join("broad.d.mts");
    let legacy = package.join("legacy").join("item.d.mts");
    let legacy_pattern = package.join("legacy-pattern.d.mts");
    std::fs::create_dir_all(legacy.parent().expect("legacy target parent"))
        .expect("create legacy imports target directory");
    for path in [&specific, &javascript, &broad] {
        std::fs::write(path, "export interface Selected {}").expect("write imports pattern target");
    }
    std::fs::write(&legacy, "export interface LegacyPrefix {}")
        .expect("write legacy imports prefix target");
    std::fs::write(&legacy_pattern, "export interface LegacyPattern {}")
        .expect("write legacy imports pattern target");
    let resolver = vue3_node_next_type_resolver();
    assert!(resolve_vue3_type_import(
        &importer.to_string_lossy(),
        "#feature/exact",
        &resolver,
    )
    .is_none());
    assert_eq!(
        resolve_vue3_type_import(
            &importer.to_string_lossy(),
            "#feature/internal/item",
            &resolver,
        ),
        Some(specific)
    );
    assert_eq!(
        resolve_vue3_type_import(
            &importer.to_string_lossy(),
            "#feature/item.js",
            &resolver,
        ),
        Some(javascript)
    );
    assert!(resolve_vue3_type_import(
        &importer.to_string_lossy(),
        "#feature/private/item",
        &resolver,
    )
    .is_none());
    assert_eq!(
        resolve_vue3_type_import(
            &importer.to_string_lossy(),
            "#prefix/item.d.mts",
            &resolver,
        ),
        Some(legacy)
    );
    assert_eq!(
        resolve_vue3_type_import(
            &importer.to_string_lossy(),
            "#legacy/item",
            &resolver,
        ),
        Some(legacy_pattern)
    );
    assert!(resolve_vue3_type_import(
        &importer.to_string_lossy(),
        "#bad-prefix/item",
        &resolver,
    )
    .is_none());

    let no_pattern_fanout =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_metadata_fanout_entries: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert!(resolve_vue3_type_import(
        &importer.to_string_lossy(),
        "#feature/item",
        &no_pattern_fanout,
    )
    .is_none());
    assert!(no_pattern_fanout
        .external_type_session
        .metadata_is_blocked());

    std::fs::write(
        package.join("package.json"),
        r##"{"imports":{"#alias":"./ok.d.mts"}}"##,
    )
    .expect("write outer imports manifest");
    let inner = package.join("inner");
    std::fs::create_dir_all(&inner).expect("create nested package scope");
    std::fs::write(inner.join("package.json"), "{}")
        .expect("write nested package scope manifest");
    let resolver = vue3_node_next_type_resolver();
    assert!(resolve_vue3_type_import(
        &inner.join("index.d.mts").to_string_lossy(),
        "#alias",
        &resolver,
    )
    .is_none());
    assert!(!resolver.external_type_session.metadata_is_blocked());

    assert!(vue3_package_import_specifier_is_safe("#alias/item"));
    for source in [
        "#",
        "#/item",
        "#alias/",
        "#alias//item",
        "#../item",
        "#types%2fitem",
    ] {
        assert!(!vue3_package_import_specifier_is_safe(source));
    }
    assert!(vue3_package_import_external_target_is_safe(
        "@scope/package/subpath"
    ));
    for target in [
        "../package",
        "package//subpath",
        "package/%2e%2e/subpath",
        "package/node_modules/subpath",
        "package/%6eode_modules/subpath",
        "package/types%2fprivate",
    ] {
        assert!(!vue3_package_import_external_target_is_safe(target));
    }
}

#[test]
fn vue3_dependency_package_imports_bound_alias_chains_and_nested_lookups() {
    let dir = tempfile::tempdir().expect("temp dir");
    let package = dir.path().join("node_modules").join("vuec-imports-budget");
    let external = package.join("node_modules").join("vuec-imports-target");
    std::fs::create_dir_all(&external).expect("create imports budget packages");
    let importer = package.join("index.d.mts");
    let leaf = package.join("leaf.d.mts");
    let external_leaf = external.join("index.d.mts");
    std::fs::write(&importer, "export {};").expect("write imports budget importer");
    std::fs::write(&leaf, "export interface Leaf { value: string }")
        .expect("write imports alias leaf");
    std::fs::write(
        package.join("package.json"),
        r##"{
            "imports":{
                "#alias":"#target",
                "#target":"./leaf.d.mts",
                "#cycle-a":"#cycle-b",
                "#cycle-b":"#cycle-a",
                "#external":["vuec-imports-target","./leaf.d.mts"]
            }
        }"##,
    )
    .expect("write imports budget manifest");
    std::fs::write(
        external.join("package.json"),
        r#"{"types":"index.d.mts"}"#,
    )
    .expect("write imports external manifest");
    std::fs::write(
        &external_leaf,
        "export interface ExternalLeaf { value: number }",
    )
    .expect("write imports external leaf");
    let filename = importer.to_string_lossy();

    let legacy_typescript = Vue3TypeResolverContext {
        typescript_version: (4, 6, 0).into(),
        module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
        ..Vue3TypeResolverContext::default()
    };
    let current_typescript = Vue3TypeResolverContext {
        typescript_version: (4, 7, 0).into(),
        ..legacy_typescript.clone()
    };
    assert!(resolve_vue3_type_import(&filename, "#alias", &legacy_typescript).is_none());
    assert_eq!(
        resolve_vue3_type_import(&filename, "#alias", &current_typescript),
        Some(leaf.clone())
    );

    let accepted =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_package_resolution_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_type_import(&filename, "#alias", &accepted),
        Some(leaf.clone())
    );
    let first_stats = accepted.external_type_session.stats();
    assert_eq!(first_stats.metadata_fanout_entries, 0);
    assert_eq!(
        resolve_vue3_type_import(&filename, "#alias", &accepted),
        Some(leaf.clone())
    );
    let cached_stats = accepted.external_type_session.stats();
    assert_eq!(cached_stats.metadata_files_read, first_stats.metadata_files_read);
    assert_eq!(
        cached_stats.metadata_resolution_path_probes,
        first_stats.metadata_resolution_path_probes
    );
    assert_eq!(cached_stats.resolution_cache_hits, 1);
    assert!(!accepted.external_type_session.metadata_is_blocked());

    let cycle = vue3_node_next_type_resolver();
    assert!(resolve_vue3_type_import(&filename, "#cycle-a", &cycle).is_none());
    assert!(!cycle.external_type_session.metadata_is_blocked());
    assert!(resolve_vue3_type_import(&filename, "#cycle-a", &cycle).is_none());
    assert_eq!(cycle.external_type_session.stats().resolution_cache_hits, 1);
    assert_eq!(
        resolve_vue3_type_import(&filename, "#alias", &cycle),
        Some(leaf)
    );

    let depth_limited =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_package_resolution_depth: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert!(resolve_vue3_type_import(&filename, "#alias", &depth_limited).is_none());
    assert!(depth_limited.external_type_session.metadata_is_blocked());

    let external_accepted =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_resolution_lookups: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert_eq!(
        resolve_vue3_type_import(&filename, "#external", &external_accepted),
        Some(external_leaf)
    );
    assert_eq!(
        external_accepted
            .external_type_session
            .stats()
            .resolution_lookups,
        2
    );

    let lookup_limited =
        vue3_node_next_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_resolution_lookups: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
    assert!(resolve_vue3_type_import(&filename, "#external", &lookup_limited).is_none());
    assert!(!lookup_limited.external_type_session.metadata_is_blocked());
    assert!(resolve_vue3_type_import(&filename, "#external", &lookup_limited).is_none());
    let stats = lookup_limited.external_type_session.stats();
    assert_eq!(stats.resolution_lookups, 1);
    assert_eq!(stats.resolution_cache_hits, 0);
}
