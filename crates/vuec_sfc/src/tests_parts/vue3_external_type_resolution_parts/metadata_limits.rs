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

#[test]
fn vue3_generated_metadata_paths_are_bounded_before_expansion() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_GENERATED_PATH_BYTES, 64 * 1024);
    assert_eq!(
        vue3_bounded_replace("*/*", "*", "123456789", 19).as_deref(),
        Some("123456789/123456789")
    );
    assert!(vue3_bounded_replace("*/*", "*", "123456789", 18).is_none());

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
fn vue3_metadata_block_propagates_from_candidate_resolution() {
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
        Vue3PackageJsonTypeResolution::Blocked
    );
    assert!(resolve_vue3_package_type_entry(&package_dir, None, &package_resolver).is_none());

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
fn vue3_package_metadata_bounds_resolution_depth() {
    assert_eq!(VUE3_EXTERNAL_TYPE_MAX_PACKAGE_RESOLUTION_DEPTH, 64);
    let dir = tempfile::tempdir().expect("temp dir");
    let accepted_package = dir.path().join("accepted");
    let rejected_package = dir.path().join("rejected");
    write_vue3_package_resolution_chain(&accepted_package, 2);
    write_vue3_package_resolution_chain(&rejected_package, 3);

    let accepted = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_package_resolution_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert!(matches!(
        resolve_vue3_package_json_type_entry(&accepted_package, None, &accepted),
        Vue3PackageJsonTypeResolution::Resolved(_)
    ));

    let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
        max_package_resolution_depth: 2,
        ..Vue3ExternalTypeLoadLimits::default()
    });
    assert_eq!(
        resolve_vue3_package_json_type_entry(&rejected_package, None, &rejected),
        Vue3PackageJsonTypeResolution::Blocked
    );
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
        r#"{
            "extends": "./base.json",
            "references": [{"path":"./referenced.json"}]
        }"#,
    )
    .expect("write root tsconfig");
    std::fs::write(
        dir.path().join("base.json"),
        r#"{"compilerOptions":{"paths":{"bounded":["./types.ts"]}}}"#,
    )
    .expect("write base tsconfig");
    std::fs::write(dir.path().join("referenced.json"), "{}")
        .expect("write referenced tsconfig");
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
        r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
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
        vec![base_file, root_file, package_file]
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
        r#"{"compilerOptions":{"typeRoots":["./types"]}}"#,
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
        r#"{"compilerOptions":{"types":[],"paths":{"alias":["./alias.ts"]}}}"#,
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
