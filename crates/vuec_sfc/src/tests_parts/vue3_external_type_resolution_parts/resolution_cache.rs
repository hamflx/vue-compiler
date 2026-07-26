    #[test]
    fn vue3_type_import_resolution_cache_caches_positive_and_negative_results() {
        let dir = tempfile::tempdir().expect("temp dir");
        let importer = dir.path().join("Comp.vue");
        let resolved = dir.path().join("types.ts");
        std::fs::write(&resolved, "export interface Props { value: string }")
            .expect("write resolved type");
        let filename = importer.to_string_lossy();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &resolver),
            Some(resolved.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &resolver),
            Some(resolved)
        );
        assert!(resolve_vue3_type_import(&filename, "./missing", &resolver).is_none());
        assert!(resolve_vue3_type_import(&filename, "./missing", &resolver).is_none());

        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 4);
        assert_eq!(stats.resolution_cache_hits, 2);
        assert!(stats.cached_resolution_weight > 0);
    }

    #[test]
    fn vue3_type_import_resolution_cache_isolates_import_and_require_modes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-mode-cache");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(&import_entry, "export interface Imported {}")
            .expect("write import entry");
        std::fs::write(&require_entry, "export interface Required {}")
            .expect("write require entry");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "vuec-mode-cache",
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            Some(import_entry.clone()),
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "vuec-mode-cache",
                Vue3TypeResolutionMode::Require,
                &resolver,
            ),
            Some(require_entry.clone()),
        );
        let first_pass = resolver.external_type_session.stats();
        assert_eq!(first_pass.resolution_lookups, 2);
        assert_eq!(first_pass.resolution_cache_hits, 0);

        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "vuec-mode-cache",
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            Some(import_entry.clone()),
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "vuec-mode-cache",
                Vue3TypeResolutionMode::Require,
                &resolver,
            ),
            Some(require_entry),
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "vuec-mode-cache", &resolver),
            Some(import_entry),
        );
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 5);
        assert_eq!(stats.resolution_cache_hits, 3);
    }

    #[test]
    fn vue3_type_import_resolution_mode_reaches_tsconfig_directory_targets() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("vuec-mode-alias");
        std::fs::create_dir_all(&package).expect("create package directory");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "mode-alias": ["node_modules/vuec-mode-alias"]
                    }
                }
            }"#,
        )
        .expect("write tsconfig paths");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.mts",
                            "require": "./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(&import_entry, "export interface Imported {}")
            .expect("write import entry");
        std::fs::write(&require_entry, "export interface Required {}")
            .expect("write require entry");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "mode-alias",
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            Some(import_entry),
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "mode-alias",
                Vue3TypeResolutionMode::Require,
                &resolver,
            ),
            Some(require_entry),
        );
    }

    #[test]
    fn vue3_type_import_resolution_cache_is_a_session_snapshot() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let late = dir.path().join("late.ts");
        let stable = dir.path().join("stable.ts");
        std::fs::write(&stable, "export type Stable = string").expect("write stable type");
        let resolver = Vue3TypeResolverContext::default();

        assert!(resolve_vue3_type_import(&filename, "./late", &resolver).is_none());
        std::fs::write(&late, "export type Late = string").expect("write late type");
        assert!(resolve_vue3_type_import(&filename, "./late", &resolver).is_none());
        assert_eq!(
            resolve_vue3_type_import(
                &filename,
                "./late",
                &Vue3TypeResolverContext::default(),
            ),
            Some(late)
        );

        assert_eq!(
            resolve_vue3_type_import(&filename, "./stable", &resolver),
            Some(stable.clone())
        );
        std::fs::remove_file(&stable).expect("remove stable type");
        assert_eq!(
            resolve_vue3_type_import(&filename, "./stable", &resolver),
            Some(stable.clone())
        );
        assert!(resolve_vue3_type_import(
            &filename,
            "./stable",
            &Vue3TypeResolverContext::default(),
        )
        .is_none());
    }

    #[test]
    fn vue3_type_import_resolution_cache_keys_lexical_importer_and_typescript_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let left = dir.path().join("left");
        let right = dir.path().join("right");
        std::fs::create_dir_all(&left).expect("create left directory");
        std::fs::create_dir_all(&right).expect("create right directory");
        let left_type = left.join("types.ts");
        let right_type = right.join("types.ts");
        std::fs::write(&left_type, "export type Side = 'left'").expect("write left type");
        std::fs::write(&right_type, "export type Side = 'right'").expect("write right type");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import(
                &left.join("Comp.vue").to_string_lossy(),
                "./types",
                &resolver,
            ),
            Some(left_type)
        );
        assert_eq!(
            resolve_vue3_type_import(
                &right.join("Comp.vue").to_string_lossy(),
                "./types",
                &resolver,
            ),
            Some(right_type)
        );

        let node_modules = dir.path().join("node_modules");
        let package_dir = node_modules.join("vuec-resolution-versioned");
        std::fs::create_dir_all(&package_dir).expect("create versioned package");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "<5.0": { "index.d.ts": ["ts4.d.ts"] },
                    ">=5.0": { "index.d.ts": ["ts5.d.ts"] }
                }
            }"#,
        )
        .expect("write versioned manifest");
        let ts4 = package_dir.join("ts4.d.ts");
        let ts5 = package_dir.join("ts5.d.ts");
        std::fs::write(&ts4, "export type Version = 4").expect("write TS 4 type");
        std::fs::write(&ts5, "export type Version = 5").expect("write TS 5 type");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let mut old_resolver = resolver.clone();
        old_resolver.typescript_version = (4, 9, 0).into();
        let mut current_resolver = resolver;
        current_resolver.typescript_version = (5, 2, 0).into();

        assert_eq!(
            resolve_vue3_type_import(
                &filename,
                "vuec-resolution-versioned",
                &old_resolver,
            ),
            Some(ts4.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(
                &filename,
                "vuec-resolution-versioned",
                &current_resolver,
            ),
            Some(ts5.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(
                &filename,
                "vuec-resolution-versioned",
                &old_resolver,
            ),
            Some(ts4)
        );
        assert_eq!(
            resolve_vue3_type_import(
                &filename,
                "vuec-resolution-versioned",
                &current_resolver,
            ),
            Some(ts5)
        );
        assert_eq!(
            current_resolver
                .external_type_session
                .stats()
                .resolution_cache_hits,
            2
        );
    }

    #[test]
    fn vue3_external_type_context_cache_keys_typescript_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package_dir = dir
            .path()
            .join("node_modules")
            .join("vuec-context-versioned");
        std::fs::create_dir_all(&package_dir).expect("create versioned package");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "<5.0": { "index.d.ts": ["ts4.d.ts"] },
                    ">=5.0": { "index.d.ts": ["ts5.d.ts"] }
                }
            }"#,
        )
        .expect("write versioned manifest");
        let ts4 = package_dir.join("ts4.d.ts");
        let ts5 = package_dir.join("ts5.d.ts");
        std::fs::write(&ts4, "export interface Versioned { legacy: string }")
            .expect("write TS 4 type");
        std::fs::write(&ts5, "export interface Versioned { current: string }")
            .expect("write TS 5 type");
        let root = dir.path().join("root.ts");
        std::fs::write(
            &root,
            "export { Versioned } from 'vuec-context-versioned'",
        )
        .expect("write versioned root");

        let old_resolver = Vue3TypeResolverContext {
            typescript_version: (4, 9, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        let current_resolver = Vue3TypeResolverContext {
            typescript_version: (5, 2, 0).into(),
            ..old_resolver.clone()
        };

        let old_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &old_resolver,
        )
        .expect("load TS 4 context");
        let current_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &current_resolver,
        )
        .expect("load TS 5 context");

        assert_eq!(
            old_context.type_sources.get("Versioned"),
            Some(&normalize_path_string(&ts4))
        );
        assert_eq!(
            current_context.type_sources.get("Versioned"),
            Some(&normalize_path_string(&ts5))
        );
        assert!(!std::sync::Arc::ptr_eq(&old_context, &current_context));
        let stats = current_resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 3);
        assert_eq!(stats.source_cache_hits, 1);
    }

    #[test]
    fn vue3_resolution_and_context_caches_key_module_suffixes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("types.first.ts");
        let second = dir.path().join("types.second.ts");
        std::fs::write(&first, "export interface Props { first: string }")
            .expect("write first suffix target");
        std::fs::write(&second, "export interface Props { second: number }")
            .expect("write second suffix target");
        let root = dir.path().join("root.ts");
        std::fs::write(&root, "export { Props } from './types'")
            .expect("write suffix-sensitive root");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let first_resolver = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([".first".to_string()]),
            ..Vue3TypeResolverContext::default()
        };
        let second_resolver = Vue3TypeResolverContext {
            module_suffixes: std::sync::Arc::from([".second".to_string()]),
            ..first_resolver.clone()
        };

        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &first_resolver),
            Some(first.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &second_resolver),
            Some(second.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &first_resolver),
            Some(first.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &second_resolver),
            Some(second.clone())
        );
        assert_eq!(
            first_resolver
                .external_type_session
                .stats()
                .resolution_cache_hits,
            2
        );

        let first_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &first_resolver,
        )
        .expect("load first suffix context");
        let second_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &second_resolver,
        )
        .expect("load second suffix context");
        assert_eq!(
            first_context.type_sources.get("Props"),
            Some(&normalize_path_string(&first))
        );
        assert_eq!(
            second_context.type_sources.get("Props"),
            Some(&normalize_path_string(&second))
        );
        assert!(!std::sync::Arc::ptr_eq(&first_context, &second_context));
    }

    #[test]
    fn vue3_external_type_context_cache_charges_key_payload() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_path = dir.path().join("empty.ts");
        std::fs::write(&source_path, "export {}").expect("write empty type source");
        let measuring = Vue3TypeResolverContext::default();
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &measuring,
        )
        .is_some());
        let expected_key_weight = source_path.as_os_str().as_encoded_bytes().len()
            + measuring.typescript_version.to_string().len()
            + measuring
                .module_suffixes
                .iter()
                .map(|suffix| std::mem::size_of::<String>() + suffix.len())
                .sum::<usize>();
        assert_eq!(
            measuring
                .external_type_session
                .stats()
                .cached_context_weight,
            expected_key_weight
        );

        let exact = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_context_cache_weight: expected_key_weight,
            max_context_cache_entry_weight: expected_key_weight,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &exact,
        )
        .is_some());
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &exact,
        )
        .is_some());
        assert_eq!(exact.external_type_session.stats().context_cache_hits, 1);

        let rejected = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_context_cache_weight: expected_key_weight - 1,
            max_context_cache_entry_weight: expected_key_weight - 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &rejected,
        )
        .is_some());
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &rejected,
        )
        .is_some());
        let rejected_stats = rejected.external_type_session.stats();
        assert_eq!(rejected_stats.context_builds, 2);
        assert_eq!(rejected_stats.context_cache_hits, 0);
        assert_eq!(rejected_stats.cached_context_weight, 0);

        let version_source = dir.path().join("version-budget.ts");
        std::fs::write(&version_source, "export interface VersionBudget {}")
            .expect("write version budget source");
        let version_prefix = "5.0.0+";
        let version_text = format!(
            "{version_prefix}{}",
            "a".repeat(nodejs_semver::MAX_LENGTH - version_prefix.len())
        );
        let version_budget = version_text.len() - 1;
        let version_limited = Vue3TypeResolverContext {
            typescript_version: nodejs_semver::Version::parse(&version_text)
                .expect("parse long TypeScript version"),
            module_suffixes: vue3_default_module_suffixes(),
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_context_build_weight: version_budget,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
        };
        assert!(vue3_external_type_context_from_path(
            &version_source,
            &mut BTreeSet::new(),
            &version_limited,
        )
        .is_none());
        let version_stats = version_limited.external_type_session.stats();
        assert_eq!(version_stats.context_builds, 0);
        assert_eq!(version_stats.import_files_read, 0);
        assert_eq!(version_stats.context_build_weight, version_budget);
    }

    #[test]
    fn vue3_type_import_resolution_cache_honors_entry_and_weight_boundaries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let first = dir.path().join("first.ts");
        let second = dir.path().join("second.ts");
        std::fs::write(&first, "export type First = string").expect("write first type");
        std::fs::write(&second, "export type Second = string").expect("write second type");

        let measuring = Vue3TypeResolverContext::default();
        assert_eq!(
            resolve_vue3_type_import(&filename, "./first", &measuring),
            Some(first.clone())
        );
        let exact_weight = measuring
            .external_type_session
            .stats()
            .cached_resolution_weight;
        assert!(exact_weight > 1);

        for (total_weight, entry_weight, expected_hits) in [
            (exact_weight, exact_weight, 1),
            (exact_weight - 1, exact_weight, 0),
            (exact_weight, exact_weight - 1, 0),
            (exact_weight, 0, 0),
        ] {
            let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
                max_resolution_cache_weight: total_weight,
                max_resolution_cache_entry_weight: entry_weight,
                ..Vue3ExternalTypeLoadLimits::default()
            });
            assert_eq!(
                resolve_vue3_type_import(&filename, "./first", &resolver),
                Some(first.clone())
            );
            assert_eq!(
                resolve_vue3_type_import(&filename, "./first", &resolver),
                Some(first.clone())
            );
            assert_eq!(
                resolver
                    .external_type_session
                    .stats()
                    .resolution_cache_hits,
                expected_hits
            );
        }

        let entry_limited =
            vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
                max_resolution_cache_entries: 1,
                ..Vue3ExternalTypeLoadLimits::default()
            });
        assert_eq!(
            resolve_vue3_type_import(&filename, "./first", &entry_limited),
            Some(first.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./second", &entry_limited),
            Some(second.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./first", &entry_limited),
            Some(first)
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./second", &entry_limited),
            Some(second)
        );
        let stats = entry_limited.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 4);
        assert_eq!(stats.resolution_cache_hits, 1);
    }

    #[test]
    fn vue3_type_import_resolution_cache_charges_lookups_before_hits() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolved = dir.path().join("types.ts");
        std::fs::write(&resolved, "export type Props = string").expect("write type");
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_resolution_lookups: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert_eq!(
            resolve_vue3_type_import(&filename, "./types", &resolver),
            Some(resolved)
        );
        assert!(resolve_vue3_type_import(&filename, "./types", &resolver).is_none());
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 1);
        assert_eq!(stats.resolution_cache_hits, 0);
    }

    #[test]
    fn vue3_type_import_resolution_cache_deduplicates_concurrent_finishes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolved = dir.path().join("types.ts");
        std::fs::write(&resolved, "export type Props = string").expect("write type");
        let resolver = Vue3TypeResolverContext::default();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                let filename = filename.clone();
                let resolver = resolver.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    resolve_vue3_type_import(&filename, "./types", &resolver)
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            assert_eq!(thread.join().expect("join resolver thread"), Some(resolved.clone()));
        }
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 8);
        assert_eq!(stats.resolution_cache_hits, 7);
        assert!(stats.cached_resolution_weight > 0);
    }

    #[test]
    fn vue3_type_import_resolution_cache_respects_metadata_blocking() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src = dir.path().join("src");
        let node_modules = dir.path().join("node_modules");
        let good_package = node_modules.join("vuec-resolution-good");
        let bad_package = node_modules.join("vuec-resolution-bad");
        std::fs::create_dir_all(&src).expect("create source directory");
        std::fs::create_dir_all(&good_package).expect("create good package");
        std::fs::create_dir_all(&bad_package).expect("create bad package");
        let package_type = good_package.join("index.d.ts");
        let relative_type = src.join("local.ts");
        std::fs::write(good_package.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write good manifest");
        std::fs::write(&package_type, "export type Good = string").expect("write package type");
        std::fs::write(bad_package.join("package.json"), "{").expect("write bad manifest");
        std::fs::write(&relative_type, "export type Local = string").expect("write local type");
        let filename = src.join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import(&filename, "vuec-resolution-good", &resolver),
            Some(package_type)
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./local", &resolver),
            Some(relative_type.clone())
        );
        assert!(resolve_vue3_type_import(&filename, "vuec-resolution-bad", &resolver).is_none());
        assert!(resolve_vue3_type_import(&filename, "vuec-resolution-good", &resolver).is_none());
        assert_eq!(
            resolve_vue3_type_import(&filename, "./local", &resolver),
            Some(relative_type)
        );
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .resolution_cache_hits,
            1
        );
    }

    #[test]
    fn vue3_type_import_resolution_cache_deduplicates_repeated_missing_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("root.ts");
        std::fs::write(
            &root,
            concat!(
                "import { Missing as Left } from './missing'\n",
                "import { Missing as Right } from './missing'\n",
                "export interface Root { left: Left; right: Right }",
            ),
        )
        .expect("write repeated missing imports");
        let resolver = Vue3TypeResolverContext::default();

        let context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load context with missing imports");
        assert!(context.declared_types.contains_key("Root"));
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 2);
        assert_eq!(stats.resolution_cache_hits, 1);
    }

    #[test]
    fn vue3_type_re_export_resolves_source_once() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("root.ts");
        let leaf = dir.path().join("leaf.ts");
        std::fs::write(&root, "export { Props } from './leaf'").expect("write re-export");
        std::fs::write(&leaf, "export interface Props { value: string }")
            .expect("write re-exported type");
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_resolution_lookups: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        let context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load re-export context");
        assert!(context.declared_types.contains_key("Props"));
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 1);
        assert_eq!(stats.resolution_cache_hits, 0);
    }
