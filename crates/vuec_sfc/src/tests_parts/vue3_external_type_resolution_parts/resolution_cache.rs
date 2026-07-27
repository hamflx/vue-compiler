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
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };

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
    fn vue3_tsconfig_directory_targets_ignore_package_exports() {
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
                "types": "./legacy.d.ts",
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
        let legacy_entry = package.join("legacy.d.ts");
        std::fs::write(&import_entry, "export interface Imported {}")
            .expect("write import entry");
        std::fs::write(&require_entry, "export interface Required {}")
            .expect("write require entry");
        std::fs::write(&legacy_entry, "export interface Legacy {}")
            .expect("write legacy entry");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let resolver = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };

        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "mode-alias",
                Vue3TypeResolutionMode::Import,
                &resolver,
            ),
            Some(legacy_entry.clone()),
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "mode-alias",
                Vue3TypeResolutionMode::Require,
                &resolver,
            ),
            Some(legacy_entry),
        );
    }

    #[test]
    fn vue3_node_esm_mode_reaches_tsconfig_module_targets() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "module": "NodeNext",
                    "moduleResolution": "NodeNext",
                    "baseUrl": ".",
                    "paths": {
                        "path-alias": ["./path-target"],
                        "explicit-alias": ["./explicit.js"]
                    }
                }
            }"#,
        )
        .expect("write NodeNext paths config");
        let path_target = dir.path().join("path-target.ts");
        let base_url_target = dir.path().join("base-target.ts");
        let explicit_target = dir.path().join("explicit.ts");
        std::fs::write(&path_target, "export interface PathProps {}")
            .expect("write paths target");
        std::fs::write(&base_url_target, "export interface BaseProps {}")
            .expect("write baseUrl target");
        std::fs::write(&explicit_target, "export interface ExplicitProps {}")
            .expect("write explicit paths target");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let node_next = vue3_type_resolver_context_for_filename(&filename);
        assert_eq!(
            node_next.module_resolution,
            Vue3TypeModuleResolutionKind::NodeNext
        );

        for source in ["path-alias", "base-target"] {
            assert!(resolve_vue3_type_import_with_mode(
                &filename,
                source,
                Vue3TypeResolutionMode::Import,
                &node_next,
            )
            .is_none());
        }
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "path-alias",
                Vue3TypeResolutionMode::Require,
                &node_next,
            ),
            Some(path_target.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "base-target",
                Vue3TypeResolutionMode::Require,
                &node_next,
            ),
            Some(base_url_target.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "explicit-alias",
                Vue3TypeResolutionMode::Import,
                &node_next,
            ),
            Some(explicit_target)
        );

        let bundler = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..node_next.clone()
        };
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "path-alias",
                Vue3TypeResolutionMode::Import,
                &bundler,
            ),
            Some(path_target)
        );
        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &filename,
                "base-target",
                Vue3TypeResolutionMode::Import,
                &bundler,
            ),
            Some(base_url_target)
        );
    }

    #[test]
    fn vue3_classic_resolution_uses_ancestor_files_and_type_packages_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let nested_dir = source_dir.join("nested");
        std::fs::create_dir_all(&nested_dir).expect("create nested source directory");
        std::fs::write(dir.path().join("tsconfig.json"), r#"{}"#)
            .expect("write project config");

        let ancestor_target = source_dir.join("choice.ts");
        std::fs::write(
            &ancestor_target,
            "export interface ChoiceProps { ancestor: string }",
        )
        .expect("write Classic ancestor target");
        let relative_directory = nested_dir.join("directory");
        std::fs::create_dir_all(&relative_directory).expect("create relative directory decoy");
        std::fs::write(
            relative_directory.join("index.ts"),
            "export interface DirectoryProps { directory: never }",
        )
        .expect("write relative directory decoy");

        let package = dir.path().join("node_modules").join("choice");
        std::fs::create_dir_all(&package).expect("create implementation package decoy");
        std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write implementation package manifest");
        let package_target = package.join("index.d.ts");
        std::fs::write(
            &package_target,
            "export interface ChoiceProps { package: never }",
        )
        .expect("write implementation package decoy");

        let package_only = dir.path().join("node_modules").join("package-only");
        std::fs::create_dir_all(&package_only).expect("create package-only decoy");
        std::fs::write(
            package_only.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write package-only manifest");
        std::fs::write(
            package_only.join("index.d.ts"),
            "export interface PackageOnlyProps { packageOnly: never }",
        )
        .expect("write package-only target");

        let types_package = dir
            .path()
            .join("node_modules")
            .join("@types")
            .join("typed-only");
        std::fs::create_dir_all(&types_package).expect("create @types fallback package");
        std::fs::write(
            types_package.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write @types fallback manifest");
        let types_target = types_package.join("index.d.ts");
        std::fs::write(
            &types_target,
            "export interface TypedOnlyProps { typedOnly: boolean }",
        )
        .expect("write @types fallback target");

        let filename = nested_dir.join("Comp.vue").to_string_lossy().to_string();
        let classic = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Classic,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&filename, "choice", &classic),
            Some(ancestor_target)
        );
        assert!(resolve_vue3_type_import(&filename, "./directory", &classic).is_none());
        assert!(resolve_vue3_type_import(&filename, "package-only", &classic).is_none());
        assert_eq!(
            resolve_vue3_type_import(&filename, "typed-only", &classic),
            Some(types_target)
        );

        let node10 = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&filename, "choice", &node10),
            Some(package_target)
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "./directory", &node10),
            Some(relative_directory.join("index.ts"))
        );
    }

    #[test]
    fn vue3_package_maps_follow_resolution_features_and_isolate_explicit_modes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "name":"project-package",
                "imports":{
                    "#alias":"./src/alias.d.ts",
                    "#/*":"./src/*.d.ts"
                },
                "exports":{"./self":"./src/self.d.ts"}
            }"##,
        )
        .expect("write project package manifest");
        let alias = source_dir.join("alias.d.ts");
        let rooted_alias = source_dir.join("rooted.d.ts");
        let self_target = source_dir.join("self.d.ts");
        std::fs::write(&alias, "export interface AliasProps {}").expect("write imports target");
        std::fs::write(&rooted_alias, "export interface RootedProps {}")
            .expect("write rooted imports target");
        std::fs::write(&self_target, "export interface SelfProps {}")
            .expect("write self-reference target");

        let package = dir.path().join("node_modules").join("mapped-package");
        std::fs::create_dir_all(&package).expect("create mapped package");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "types":"./legacy.d.ts",
                "exports":{".":{"types":"./modern.d.ts"}}
            }"#,
        )
        .expect("write mapped package manifest");
        let legacy = package.join("legacy.d.ts");
        let modern = package.join("modern.d.ts");
        std::fs::write(&legacy, "export interface LegacyProps {}")
            .expect("write legacy package target");
        std::fs::write(&modern, "export interface ModernProps {}")
            .expect("write modern package target");

        let self_dependency = dir.path().join("node_modules").join("project-package");
        std::fs::create_dir_all(&self_dependency).expect("create self-name dependency");
        std::fs::write(
            self_dependency.join("package.json"),
            r#"{"types":"self.d.ts"}"#,
        )
        .expect("write self-name dependency manifest");
        let legacy_self = self_dependency.join("self.d.ts");
        std::fs::write(&legacy_self, "export interface LegacySelfProps {}")
            .expect("write self-name dependency target");

        let filename = source_dir.join("Comp.vue").to_string_lossy().to_string();
        let node10 = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&filename, "mapped-package", &node10),
            Some(legacy.clone())
        );
        assert!(resolve_vue3_type_import(&filename, "#alias", &node10).is_none());
        assert_eq!(
            resolve_vue3_type_import(&filename, "project-package/self", &node10),
            Some(legacy_self.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_with_explicit_mode(
                &filename,
                "mapped-package",
                Vue3TypeResolutionMode::Import,
                &node10,
            ),
            Some(modern.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_with_explicit_mode(
                &filename,
                "#alias",
                Vue3TypeResolutionMode::Import,
                &node10,
            ),
            Some(alias.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_with_explicit_mode(
                &filename,
                "project-package/self",
                Vue3TypeResolutionMode::Import,
                &node10,
            ),
            Some(self_target.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "mapped-package", &node10),
            Some(legacy.clone())
        );
        assert_eq!(
            resolve_vue3_type_import_with_explicit_mode(
                &filename,
                "mapped-package",
                Vue3TypeResolutionMode::Import,
                &node10,
            ),
            Some(modern.clone())
        );
        let node10_stats = node10.external_type_session.stats();
        assert_eq!(node10_stats.resolution_lookups, 8);
        assert_eq!(node10_stats.resolution_cache_hits, 2);

        let node_next = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&filename, "mapped-package", &node_next),
            Some(modern.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "#alias", &node_next),
            Some(alias.clone())
        );
        assert_eq!(
            resolve_vue3_type_import(&filename, "project-package/self", &node_next),
            Some(self_target.clone())
        );

        let typescript_5_2 = Vue3TypeResolverContext {
            typescript_version: (5, 2, 2).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import_with_explicit_mode(
                &filename,
                "mapped-package",
                Vue3TypeResolutionMode::Import,
                &typescript_5_2,
            ),
            Some(legacy.clone())
        );
        let typescript_5_3 = Vue3TypeResolverContext {
            typescript_version: (5, 3, 0).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import_with_explicit_mode(
                &filename,
                "mapped-package",
                Vue3TypeResolutionMode::Import,
                &typescript_5_3,
            ),
            Some(modern.clone())
        );

        for (version, module_resolution, explicit_mode, expected) in [
            (
                (5, 9, 0),
                Vue3TypeModuleResolutionKind::NodeNext,
                false,
                None,
            ),
            (
                (6, 0, 0),
                Vue3TypeModuleResolutionKind::Node16,
                false,
                None,
            ),
            (
                (6, 0, 0),
                Vue3TypeModuleResolutionKind::NodeNext,
                false,
                Some(rooted_alias.clone()),
            ),
            (
                (6, 0, 0),
                Vue3TypeModuleResolutionKind::Bundler,
                false,
                Some(rooted_alias.clone()),
            ),
            (
                (6, 0, 0),
                Vue3TypeModuleResolutionKind::Node10,
                false,
                None,
            ),
            (
                (6, 0, 0),
                Vue3TypeModuleResolutionKind::Node10,
                true,
                Some(rooted_alias.clone()),
            ),
        ] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                module_resolution,
                ..Vue3TypeResolverContext::default()
            };
            let actual = if explicit_mode {
                resolve_vue3_type_import_with_explicit_mode(
                    &filename,
                    "#/rooted",
                    Vue3TypeResolutionMode::Import,
                    &resolver,
                )
            } else {
                resolve_vue3_type_import(&filename, "#/rooted", &resolver)
            };
            assert_eq!(actual, expected, "TypeScript {version:?} {module_resolution:?}");
        }

        let bundler_maps_disabled = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            resolve_package_json_exports: Some(false),
            resolve_package_json_imports: Some(false),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&filename, "mapped-package", &bundler_maps_disabled),
            Some(legacy.clone())
        );
        assert!(resolve_vue3_type_import(&filename, "#alias", &bundler_maps_disabled).is_none());
        assert_eq!(
            resolve_vue3_type_import(
                &filename,
                "project-package/self",
                &bundler_maps_disabled,
            ),
            Some(self_target.clone())
        );

        for module_resolution in [
            Vue3TypeModuleResolutionKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext,
        ] {
            let node_maps_disabled = Vue3TypeResolverContext {
                module_resolution,
                resolve_package_json_exports: Some(false),
                resolve_package_json_imports: Some(false),
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_type_import(&filename, "mapped-package", &node_maps_disabled),
                Some(modern.clone()),
                "{module_resolution:?} should preserve exports"
            );
            assert_eq!(
                resolve_vue3_type_import(&filename, "#alias", &node_maps_disabled),
                Some(alias.clone()),
                "{module_resolution:?} should preserve imports"
            );
            assert_eq!(
                resolve_vue3_type_import(
                    &filename,
                    "project-package/self",
                    &node_maps_disabled,
                ),
                Some(self_target.clone()),
                "{module_resolution:?} should preserve self-name resolution"
            );
        }

        let conditional = dir.path().join("node_modules").join("conditional-package");
        std::fs::create_dir_all(&conditional).expect("create conditional package");
        std::fs::write(
            conditional.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "node": "./node.d.ts",
                        "import": "./import.d.ts",
                        "default": "./default.d.ts"
                    }
                }
            }"#,
        )
        .expect("write conditional package manifest");
        let node_entry = conditional.join("node.d.ts");
        let import_entry = conditional.join("import.d.ts");
        std::fs::write(&node_entry, "export interface NodeProps {}")
            .expect("write node condition target");
        std::fs::write(&import_entry, "export interface ImportProps {}")
            .expect("write import condition target");

        assert_eq!(
            resolve_vue3_type_import(&filename, "conditional-package", &node_next),
            Some(node_entry)
        );
        let bundler = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&filename, "conditional-package", &bundler),
            Some(import_entry)
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
    fn vue3_resolution_and_context_caches_key_module_resolution() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("types.ts");
        std::fs::write(&target, "export interface Props { value: string }")
            .expect("write resolution target");
        let root = dir.path().join("root.mts");
        std::fs::write(&root, "export { Props } from './types'")
            .expect("write resolution-sensitive root");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let session = Vue3ExternalTypeLoadSession::default();
        let node10 = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            external_type_session: session.clone(),
            ..Vue3TypeResolverContext::default()
        };
        let node_next = Vue3TypeResolverContext {
            module_resolution: Vue3TypeModuleResolutionKind::NodeNext,
            ..node10.clone()
        };

        for _ in 0..2 {
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &filename,
                    "./types",
                    Vue3TypeResolutionMode::Import,
                    &node10,
                ),
                Some(target.clone())
            );
            assert!(resolve_vue3_type_import_with_mode(
                &filename,
                "./types",
                Vue3TypeResolutionMode::Import,
                &node_next,
            )
            .is_none());
        }
        assert_eq!(session.stats().resolution_cache_hits, 2);

        let node10_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &node10,
        )
        .expect("load Node10 context");
        let node_next_context = vue3_external_type_context_from_path(
            &root,
            &mut BTreeSet::new(),
            &node_next,
        )
        .expect("load NodeNext context");
        assert_eq!(
            node10_context.type_sources.get("Props"),
            Some(&normalize_path_string(&target))
        );
        assert!(!node_next_context.type_sources.contains_key("Props"));
        assert!(!std::sync::Arc::ptr_eq(
            &node10_context,
            &node_next_context
        ));
    }

    #[test]
    fn vue3_context_caches_key_effective_module_and_source_modes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("conditional-module");
        std::fs::create_dir_all(&package).expect("create conditional package");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports": {
                    ".": {
                        "types": {
                            "import": "./import.d.ts",
                            "require": "./require.d.ts"
                        }
                    }
                }
            }"#,
        )
        .expect("write conditional manifest");
        let import_entry = package.join("import.d.ts");
        let require_entry = package.join("require.d.ts");
        std::fs::write(&import_entry, "export interface Props { imported: string }")
            .expect("write import branch");
        std::fs::write(&require_entry, "export interface Props { required: string }")
            .expect("write require branch");
        let root = dir.path().join("root.d.ts");
        std::fs::write(&root, "export { Props } from 'conditional-module'")
            .expect("write mode-sensitive root");

        for esm_first in [false, true] {
            let session = Vue3ExternalTypeLoadSession::default();
            let commonjs = Vue3TypeResolverContext {
                typescript_version: (6, 0, 0).into(),
                module_resolution: Vue3TypeModuleResolutionKind::Bundler,
                module: Some(Vue3TypeModuleKind::CommonJs),
                external_type_session: session.clone(),
                ..Vue3TypeResolverContext::default()
            };
            let esnext = Vue3TypeResolverContext {
                module: Some(Vue3TypeModuleKind::EcmaScript),
                ..commonjs.clone()
            };
            assert_ne!(commonjs, esnext);

            let load = |resolver: &Vue3TypeResolverContext| {
                vue3_external_type_context_from_path(
                    &root,
                    &mut BTreeSet::new(),
                    resolver,
                )
                .expect("load mode-sensitive context")
            };
            let (commonjs_context, esnext_context) = if esm_first {
                let esnext_context = load(&esnext);
                let commonjs_context = load(&commonjs);
                (commonjs_context, esnext_context)
            } else {
                let commonjs_context = load(&commonjs);
                let esnext_context = load(&esnext);
                (commonjs_context, esnext_context)
            };

            assert_eq!(
                commonjs_context.type_sources.get("Props"),
                Some(&normalize_path_string(&require_entry)),
            );
            assert_eq!(
                esnext_context.type_sources.get("Props"),
                Some(&normalize_path_string(&import_entry)),
            );
            assert!(!std::sync::Arc::ptr_eq(
                &commonjs_context,
                &esnext_context,
            ));
            assert_eq!(session.stats().context_builds, 4);
            assert_eq!(session.stats().context_cache_hits, 0);

            assert!(std::sync::Arc::ptr_eq(&commonjs_context, &load(&commonjs)));
            assert!(std::sync::Arc::ptr_eq(&esnext_context, &load(&esnext)));
            assert_eq!(session.stats().context_cache_hits, 2);
        }
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
            + std::mem::size_of::<Vue3TypeModuleResolutionKind>()
            + std::mem::size_of::<Vue3TypeModuleKind>()
            + std::mem::size_of::<bool>()
            + std::mem::size_of::<Vue3PackageJsonResolutionFeatures>() * 2
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
            module_resolution: Vue3TypeModuleResolutionKind::Node10,
            module: None,
            allow_js: false,
            custom_conditions: Vue3CustomConditionSet::default(),
            resolve_package_json_exports: None,
            resolve_package_json_imports: None,
            active_package_json_features: None,
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
    fn vue3_type_import_resolution_cache_isolates_allow_js_self_name_semantics() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let output_dir = dir.path().join("dist");
        for directory in [&source_dir, &output_dir] {
            std::fs::create_dir_all(directory).expect("create project directory");
        }
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-allow-js-cache",
                "exports":{
                    ".":{
                        "import":"./dist/runtime.js",
                        "default":"./types.d.ts"
                    }
                }
            }"#,
        )
        .expect("write self-name package manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions":{
                    "module":"ESNext",
                    "moduleResolution":"Bundler",
                    "rootDir":"./src",
                    "outDir":"./dist"
                }
            }"#,
        )
        .expect("write self-name project config");
        let importer = source_dir.join("consumer.ts");
        std::fs::write(&importer, "export {};").expect("write self-name importer");
        let javascript = source_dir.join("runtime.js");
        std::fs::write(&javascript, "export const runtime = true;")
            .expect("write project JavaScript input");
        let emitted_declaration = output_dir.join("runtime.d.ts");
        std::fs::write(
            &emitted_declaration,
            "export interface EmittedDeclarationProps {}",
        )
        .expect("write emitted declaration");
        std::fs::write(
            dir.path().join("types.d.ts"),
            "export interface FallbackDeclarationProps {}",
        )
        .expect("write declaration fallback");
        let disabled = Vue3TypeResolverContext {
            typescript_version: (5, 2, 2).into(),
            module_resolution: Vue3TypeModuleResolutionKind::Bundler,
            ..Vue3TypeResolverContext::default()
        };
        let enabled = Vue3TypeResolverContext {
            allow_js: true,
            ..disabled.clone()
        };

        assert_eq!(
            resolve_vue3_type_import_with_mode(
                &importer.to_string_lossy(),
                "vuec-allow-js-cache",
                Vue3TypeResolutionMode::Import,
                &disabled,
            ),
            Some(emitted_declaration),
        );
        for _ in 0..2 {
            assert_eq!(
                resolve_vue3_type_import_with_mode(
                    &importer.to_string_lossy(),
                    "vuec-allow-js-cache",
                    Vue3TypeResolutionMode::Import,
                    &enabled,
                ),
                Some(javascript.clone()),
            );
        }
        let stats = disabled.external_type_session.stats();
        assert_eq!(stats.resolution_lookups, 3);
        assert_eq!(stats.resolution_cache_hits, 1);
        assert!(!disabled.external_type_session.metadata_is_blocked());
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
