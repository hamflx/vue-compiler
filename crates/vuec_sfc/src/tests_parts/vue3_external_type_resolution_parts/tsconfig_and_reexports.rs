    #[test]
    fn vue3_compile_script_resolves_tsconfig_path_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let types_pkg = node_modules.join("vuec-tsconfig-pkg");
        std::fs::create_dir_all(&types_pkg).expect("create package");
        std::fs::write(types_pkg.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write package manifest");
        std::fs::write(
            types_pkg.join("index.d.ts"),
            "export type PackageProps = { packaged: boolean }",
        )
        .expect("write package types");

        std::fs::create_dir_all(dir.path().join("web")).expect("create web dir");
        std::fs::create_dir_all(dir.path().join("empty")).expect("create empty dir");
        std::fs::create_dir_all(dir.path().join("tsconfigs")).expect("create tsconfigs dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("src").join("views")).expect("create views dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": [],
                "compilerOptions": {
                    "paths": {
                        "bar": ["./pp.ts"],
                        "user": ["./user.ts"],
                        "@/*": ["${configDir}/src/*"]
                    }
                },
                "references": [
                    { "path": "./tsconfig.app.json" },
                    { "path": "./web" },
                    { "path": "./empty" },
                    { "path": "./noexists-should-ignore" }
                ]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("tsconfig.app.json"),
            r#"{
                "include": ["**/*.ts", "**/*.vue"],
                "extends": ["./tsconfigs/base.json"]
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.path().join("tsconfigs").join("base.json"),
            r#"{
                "include": ["${configDir}/src/**/*.ts", "${configDir}/src/**/*.vue"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("web").join("tsconfig.json"),
            r#"{
                "include": ["../**/*.ts", "../**/*.vue"],
                "compilerOptions": {
                    "composite": true
                },
                "references": [
                    { "path": "../tsconfig.json" }
                ]
            }"#,
        )
        .expect("write web tsconfig");
        std::fs::write(
            dir.path().join("empty").join("tsconfig.json"),
            r#"{"compilerOptions":{"composite":true}}"#,
        )
        .expect("write empty tsconfig");
        std::fs::write(
            dir.path().join("pp.ts"),
            "export type PathProps = { bar: string }",
        )
        .expect("write root path type");
        std::fs::write(
            dir.path().join("user.ts"),
            "export type UserProps = { user: string }",
        )
        .expect("write referenced type");
        std::fs::write(
            dir.path().join("src").join("types.ts"),
            "export type BaseProps = { foo?: string; count: number }",
        )
        .expect("write configDir type");
        std::fs::write(
            dir.path().join("src").join("views").join("Aliased.vue"),
            "<script lang=\"ts\">export type VueProps = { fromVue: string }</script>",
        )
        .expect("write aliased vue");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { PackageProps } from 'vuec-tsconfig-pkg'
import type { PathProps } from 'bar'
import type { UserProps } from 'user'
import type { BaseProps } from '@/types.ts'
import type { VueProps } from '@/views/Aliased.vue'
const props = defineProps<PackageProps & PathProps & UserProps & BaseProps & VueProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected_prop in [
            "packaged: { type: Boolean, required: true }",
            "bar: { type: String, required: true }",
            "user: { type: String, required: true }",
            "foo: { type: String, required: false }",
            "count: { type: Number, required: true }",
            "fromVue: { type: String, required: true }",
        ] {
            assert!(script.content.contains(expected_prop), "{}", script.content);
        }

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            types_pkg.join("index.d.ts"),
            dir.path().join("pp.ts"),
            dir.path().join("user.ts"),
            dir.path().join("src").join("types.ts"),
            dir.path().join("src").join("views").join("Aliased.vue"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_rooted_and_parent_include_globs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config_dir = dir.path().join("project").join("config");
        let absolute_types = dir.path().join("absolute-types");
        let shared_types = dir.path().join("shared");
        for directory in [&config_dir, &absolute_types, &shared_types] {
            std::fs::create_dir_all(directory).expect("create include fixture directory");
        }

        let absolute_global = absolute_types.join("absolute.d.ts");
        let parent_global = shared_types.join("parent.d.ts");
        std::fs::write(
            &absolute_global,
            "declare interface AbsoluteGlobalProps { absoluteValue: string }",
        )
        .expect("write rooted include declaration");
        std::fs::write(
            &parent_global,
            "declare interface ParentGlobalProps { parentValue?: number }",
        )
        .expect("write parent include declaration");

        let absolute_pattern = format!(r"{}\**\*.d.ts", absolute_types.to_string_lossy());
        let tsconfig = serde_json::json!({
            "include": [absolute_pattern, r"..\..\shared\**\*.d.ts"],
            "compilerOptions": { "types": [] }
        });
        std::fs::write(
            config_dir.join("tsconfig.json"),
            serde_json::to_vec(&tsconfig).expect("serialize include config"),
        )
        .expect("write include config");

        let filename = config_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<AbsoluteGlobalProps & ParentGlobalProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("absoluteValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("parentValue: { type: Number, required: false }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [absolute_global, parent_global]
                .into_iter()
                .map(|path| normalize_path_string(&path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_compile_script_resolves_base_url_after_paths_and_before_packages() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(source_dir.join("components"))
            .expect("create project source directory");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "baseUrl": "./src",
                    "paths": {
                        "base-choice": ["./missing.ts"],
                        "mapped-choice": ["./mapped.ts"]
                    }
                }
            }"#,
        )
        .expect("write baseUrl config");
        let base_choice = source_dir.join("base-choice.ts");
        let mapped_choice = source_dir.join("mapped.ts");
        std::fs::write(
            &base_choice,
            "export interface BaseChoiceProps { baseValue: string }",
        )
        .expect("write baseUrl target");
        std::fs::write(
            &mapped_choice,
            "export interface MappedChoiceProps { mappedValue: boolean }",
        )
        .expect("write paths target");

        let base_decoy = dir.path().join("node_modules").join("base-choice");
        let package_fallback = dir.path().join("node_modules").join("package-fallback");
        for package in [&base_decoy, &package_fallback] {
            std::fs::create_dir_all(package).expect("create dependency package");
            std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
                .expect("write dependency manifest");
        }
        std::fs::write(
            base_decoy.join("index.d.ts"),
            "export interface BaseChoiceProps { wrongPackagePriority: never }",
        )
        .expect("write baseUrl package decoy");
        let package_entry = package_fallback.join("index.d.ts");
        std::fs::write(
            &package_entry,
            "export interface PackageProps { packageValue: number }",
        )
        .expect("write package fallback target");

        let filename = source_dir.join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { BaseChoiceProps } from 'base-choice'
import type { MappedChoiceProps } from 'mapped-choice'
import type { PackageProps } from 'package-fallback'
defineProps<BaseChoiceProps & MappedChoiceProps & PackageProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "baseValue: { type: String, required: true }",
            "mappedValue: { type: Boolean, required: true }",
            "packageValue: { type: Number, required: true }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongPackagePriority"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [base_choice, mapped_choice, package_entry]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_compile_script_inherits_and_overrides_base_url_without_reference_leaks() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config_dir = dir.path().join("configs");
        let inherited_source_dir = dir.path().join("sources");
        let inherited_project = dir.path().join("inherited-project");
        let override_project = dir.path().join("override-project");
        let referenced_project = dir.path().join("referenced-project");
        let reference_consumer = dir.path().join("reference-consumer");
        for directory in [
            &config_dir,
            &inherited_source_dir,
            &inherited_project,
            &override_project.join("local"),
            &referenced_project.join("types"),
            &reference_consumer,
        ] {
            std::fs::create_dir_all(directory).expect("create baseUrl inheritance fixture");
        }
        std::fs::write(
            config_dir.join("base.json"),
            r#"{"compilerOptions":{"baseUrl":"../sources"}}"#,
        )
        .expect("write inherited baseUrl config");
        std::fs::write(
            inherited_project.join("tsconfig.json"),
            r#"{"extends":"../configs/base.json"}"#,
        )
        .expect("write inherited project config");
        std::fs::write(
            override_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/base.json",
                "compilerOptions":{"baseUrl":"./local"}
            }"#,
        )
        .expect("write overriding project config");
        std::fs::write(
            referenced_project.join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "baseUrl": "./types",
                    "paths": { "choice": ["${configDir}/types/choice.ts"] }
                }
            }"#,
        )
        .expect("write referenced project config with a colliding path mapping");
        std::fs::write(
            reference_consumer.join("tsconfig.json"),
            r#"{"references":[{"path":"../referenced-project"}]}"#,
        )
        .expect("write reference consumer config");
        let inherited_target = inherited_source_dir.join("choice.ts");
        let override_target = override_project.join("local").join("choice.ts");
        let referenced_target = referenced_project.join("types").join("choice.ts");
        std::fs::write(
            &inherited_target,
            "export interface ChoiceProps { inheritedValue: string }",
        )
        .expect("write inherited baseUrl target");
        std::fs::write(
            &override_target,
            "export interface ChoiceProps { overrideValue: number }",
        )
        .expect("write overriding baseUrl target");
        std::fs::write(
            &referenced_target,
            "export interface ChoiceProps { referencedValue: boolean }",
        )
        .expect("write referenced baseUrl target");
        let package = dir.path().join("node_modules").join("choice");
        std::fs::create_dir_all(&package).expect("create reference fallback package");
        std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write reference fallback manifest");
        let package_target = package.join("index.d.ts");
        std::fs::write(
            &package_target,
            "export interface ChoiceProps { packageValue: boolean }",
        )
        .expect("write reference fallback types");

        let compile = |filename: &Path| {
            let source = r#"<script setup lang="ts">
import type { ChoiceProps } from 'choice'
defineProps<ChoiceProps>()
</script>"#;
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename.to_string_lossy(), source);
            compiler.compile_script(&descriptor, SfcScriptCompileOptions::default())
        };
        let inherited = compile(&inherited_project.join("Comp.vue"));
        let overridden = compile(&override_project.join("Comp.vue"));
        let referenced = compile(&reference_consumer.join("Comp.vue"));

        assert!(inherited.errors.is_empty(), "{:?}", inherited.errors);
        assert!(overridden.errors.is_empty(), "{:?}", overridden.errors);
        assert!(referenced.errors.is_empty(), "{:?}", referenced.errors);
        assert!(
            inherited
                .content
                .contains("inheritedValue: { type: String, required: true }"),
            "{}",
            inherited.content
        );
        assert!(
            overridden
                .content
                .contains("overrideValue: { type: Number, required: true }"),
            "{}",
            overridden.content
        );
        assert!(
            referenced
                .content
                .contains("packageValue: { type: Boolean, required: true }"),
            "{}",
            referenced.content
        );
        assert!(!referenced.content.contains("referencedValue"));
        assert_eq!(
            inherited.deps,
            vec![normalize_path_string(&inherited_target)]
        );
        assert_eq!(
            overridden.deps,
            vec![normalize_path_string(&override_target)]
        );
        assert_eq!(referenced.deps, vec![normalize_path_string(&package_target)]);
    }

    #[test]
    fn vue3_base_url_lookup_uses_the_nearest_config_and_stops_at_typescript_7() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let package = dir.path().join("node_modules").join("choice");
        std::fs::create_dir_all(&source_dir).expect("create baseUrl source directory");
        std::fs::create_dir_all(&package).expect("create fallback package");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "baseUrl": "./src",
                    "paths": {
                        "nested-choice": ["${configDir}/ancestor-choice.ts"]
                    }
                }
            }"#,
        )
        .expect("write baseUrl config");
        let base_url_target = source_dir.join("choice.ts");
        std::fs::write(
            &base_url_target,
            "export interface ChoiceProps { baseUrlValue: string }",
        )
        .expect("write baseUrl target");
        let relative_target = dir.path().join("relative.ts");
        std::fs::write(
            &relative_target,
            "export interface RelativeProps { relativeValue: boolean }",
        )
        .expect("write relative target");
        std::fs::write(
            source_dir.join("relative.ts"),
            "export interface RelativeProps { wrongBaseUrlValue: never }",
        )
        .expect("write baseUrl relative decoy");
        std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write fallback package manifest");
        let package_target = package.join("index.d.ts");
        std::fs::write(
            &package_target,
            "export interface ChoiceProps { packageValue: number }",
        )
        .expect("write fallback package types");
        let nested_package = dir.path().join("node_modules").join("nested-choice");
        std::fs::create_dir_all(&nested_package).expect("create nested fallback package");
        std::fs::write(
            nested_package.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write nested fallback manifest");
        let nested_package_target = nested_package.join("index.d.ts");
        std::fs::write(
            &nested_package_target,
            "export interface NestedChoiceProps { packageValue: string }",
        )
        .expect("write nested fallback types");
        let ancestor_paths_target = dir.path().join("ancestor-choice.ts");
        std::fs::write(
            &ancestor_paths_target,
            "export interface NestedChoiceProps { ancestorPathLeak: never }",
        )
        .expect("write ancestor paths decoy");
        let importer = dir.path().join("Comp.vue").to_string_lossy().to_string();

        for version in [(5, 9, 0), (6, 0, 0)] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_type_import(&importer, "choice", &resolver),
                Some(base_url_target.clone())
            );
            assert!(!resolver.external_type_session.metadata_is_blocked());
        }
        let typescript_7 = Vue3TypeResolverContext {
            typescript_version: (7, 0, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&importer, r".\relative", &typescript_7),
            Some(relative_target)
        );
        assert_eq!(
            resolve_vue3_type_import(&importer, "choice", &typescript_7),
            Some(package_target.clone())
        );

        let nested_dir = dir.path().join("nested");
        std::fs::create_dir_all(&nested_dir).expect("create nested project");
        std::fs::write(nested_dir.join("tsconfig.json"), "{}")
            .expect("write nearest config without baseUrl");
        let nested_importer = nested_dir.join("Comp.vue").to_string_lossy().to_string();
        let nested_typescript_5 = Vue3TypeResolverContext {
            typescript_version: (5, 9, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&nested_importer, "choice", &nested_typescript_5),
            Some(package_target)
        );
        assert_eq!(
            resolve_vue3_type_import(
                &nested_importer,
                "nested-choice",
                &nested_typescript_5,
            ),
            Some(nested_package_target)
        );
        assert!(!typescript_7.external_type_session.metadata_is_blocked());
        assert!(!nested_typescript_5
            .external_type_session
            .metadata_is_blocked());
    }

    #[test]
    fn vue3_tsconfig_paths_use_effective_base_url_and_replace_inherited_options() {
        let dir = tempfile::tempdir().expect("temp dir");
        let configs = dir.path().join("configs");
        let parent_source = dir.path().join("parent-source");
        let inherited_base_url_project = dir.path().join("inherited-base-url");
        let overridden_base_url_project = dir.path().join("overridden-base-url");
        let cleared_base_url_project = dir.path().join("cleared-base-url");
        let replacement_project = dir.path().join("replacement");
        let empty_paths_project = dir.path().join("empty-paths");
        let null_paths_project = dir.path().join("null-paths");
        let multiple_extends_project = dir.path().join("multiple-extends");
        for directory in [
            &configs,
            &parent_source,
            &inherited_base_url_project,
            &overridden_base_url_project.join("source"),
            &cleared_base_url_project,
            &replacement_project,
            &empty_paths_project,
            &null_paths_project,
            &multiple_extends_project,
        ] {
            std::fs::create_dir_all(directory).expect("create paths inheritance fixture");
        }

        std::fs::write(
            configs.join("base-url.json"),
            r#"{"compilerOptions":{"baseUrl":"../parent-source"}}"#,
        )
        .expect("write inherited baseUrl config");
        std::fs::write(
            inherited_base_url_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/base-url.json",
                "compilerOptions":{"paths":{"choice":["choice"]}}
            }"#,
        )
        .expect("write child paths config");
        let inherited_base_url_target = parent_source.join("choice.ts");
        let typescript_7_target = inherited_base_url_project.join("choice.ts");
        std::fs::write(&inherited_base_url_target, "export interface Choice { inherited: string }")
            .expect("write inherited baseUrl target");
        std::fs::write(&typescript_7_target, "export interface Choice { typescript7: string }")
            .expect("write TypeScript 7 paths target");
        let inherited_importer = inherited_base_url_project
            .join("Comp.vue")
            .to_string_lossy()
            .to_string();
        for version in [(5, 9, 0), (6, 0, 0)] {
            let resolver = Vue3TypeResolverContext {
                typescript_version: version.into(),
                ..Vue3TypeResolverContext::default()
            };
            assert_eq!(
                resolve_vue3_type_import(&inherited_importer, "choice", &resolver),
                Some(inherited_base_url_target.clone())
            );
        }
        let typescript_7 = Vue3TypeResolverContext {
            typescript_version: (7, 0, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        assert_eq!(
            resolve_vue3_type_import(&inherited_importer, "choice", &typescript_7),
            Some(typescript_7_target)
        );

        std::fs::write(
            configs.join("paths.json"),
            r#"{
                "compilerOptions":{
                    "baseUrl":"../parent-source",
                    "paths":{"rebased":["rebased"]}
                }
            }"#,
        )
        .expect("write inherited paths config");
        std::fs::write(
            overridden_base_url_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/paths.json",
                "compilerOptions":{"baseUrl":"./source"}
            }"#,
        )
        .expect("write overriding baseUrl config");
        std::fs::write(
            parent_source.join("rebased.ts"),
            "export interface Rebased { wrongParentBase: never }",
        )
        .expect("write old baseUrl decoy");
        let overridden_base_url_target = overridden_base_url_project
            .join("source")
            .join("rebased.ts");
        std::fs::write(
            &overridden_base_url_target,
            "export interface Rebased { childBase: number }",
        )
        .expect("write overridden baseUrl target");
        assert_eq!(
            resolve_vue3_type_import(
                &overridden_base_url_project
                    .join("Comp.vue")
                    .to_string_lossy(),
                "rebased",
                &Vue3TypeResolverContext::default(),
            ),
            Some(overridden_base_url_target)
        );

        std::fs::write(
            cleared_base_url_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/paths.json",
                "compilerOptions":{"baseUrl":null}
            }"#,
        )
        .expect("write cleared baseUrl config");
        let cleared_base_url_target = configs.join("rebased.ts");
        std::fs::write(
            &cleared_base_url_target,
            "export interface Rebased { declaredPathsBase: boolean }",
        )
        .expect("write declared paths base target");
        assert_eq!(
            resolve_vue3_type_import(
                &cleared_base_url_project
                    .join("Comp.vue")
                    .to_string_lossy(),
                "rebased",
                &Vue3TypeResolverContext::default(),
            ),
            Some(cleared_base_url_target)
        );

        std::fs::write(
            configs.join("replace.json"),
            r#"{"compilerOptions":{"paths":{"parent-only":["${configDir}/parent.ts"]}}}"#,
        )
        .expect("write replaceable paths config");
        std::fs::write(
            replacement_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/replace.json",
                "compilerOptions":{"paths":{"child-only":["./child.ts"]}}
            }"#,
        )
        .expect("write replacing paths config");
        let child_target = replacement_project.join("child.ts");
        std::fs::write(&child_target, "export interface Child { value: boolean }")
            .expect("write replacement target");
        std::fs::write(
            replacement_project.join("parent.ts"),
            "export interface Parent { shouldBeHidden: never }",
        )
        .expect("write replaced parent target");
        let replacement_importer = replacement_project
            .join("Comp.vue")
            .to_string_lossy()
            .to_string();
        let replacement_resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            resolve_vue3_type_import(&replacement_importer, "child-only", &replacement_resolver),
            Some(child_target)
        );
        assert!(resolve_vue3_type_import(
            &replacement_importer,
            "parent-only",
            &replacement_resolver,
        )
        .is_none());

        std::fs::write(
            empty_paths_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/replace.json",
                "compilerOptions":{"paths":{}}
            }"#,
        )
        .expect("write empty paths override");
        assert!(resolve_vue3_type_import(
            &empty_paths_project.join("Comp.vue").to_string_lossy(),
            "parent-only",
            &Vue3TypeResolverContext::default(),
        )
        .is_none());
        std::fs::write(
            null_paths_project.join("tsconfig.json"),
            r#"{
                "extends":"../configs/replace.json",
                "compilerOptions":{"paths":null}
            }"#,
        )
        .expect("write null paths override");
        assert!(resolve_vue3_type_import(
            &null_paths_project.join("Comp.vue").to_string_lossy(),
            "parent-only",
            &Vue3TypeResolverContext::default(),
        )
        .is_none());

        std::fs::write(
            configs.join("first.json"),
            r#"{"compilerOptions":{"paths":{"first-only":["${configDir}/first.ts"]}}}"#,
        )
        .expect("write first extended paths config");
        std::fs::write(
            configs.join("second.json"),
            r#"{"compilerOptions":{"paths":{"second-only":["${configDir}/second.ts"]}}}"#,
        )
        .expect("write second extended paths config");
        std::fs::write(
            multiple_extends_project.join("tsconfig.json"),
            r#"{"extends":["../configs/first.json","../configs/second.json"]}"#,
        )
        .expect("write multiple extends config");
        let second_target = multiple_extends_project.join("second.ts");
        std::fs::write(
            multiple_extends_project.join("first.ts"),
            "export interface First { shouldBeHidden: never }",
        )
        .expect("write first extends decoy");
        std::fs::write(&second_target, "export interface Second { value: string }")
            .expect("write second extends target");
        let multiple_importer = multiple_extends_project
            .join("Comp.vue")
            .to_string_lossy()
            .to_string();
        let multiple_resolver = Vue3TypeResolverContext::default();
        assert!(resolve_vue3_type_import(
            &multiple_importer,
            "first-only",
            &multiple_resolver,
        )
        .is_none());
        assert_eq!(
            resolve_vue3_type_import(&multiple_importer, "second-only", &multiple_resolver),
            Some(second_target)
        );
    }

    #[test]
    fn vue3_tsconfig_paths_choose_wildcards_by_longest_prefix() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "a*bcd": ["./longer-total-pattern.ts"],
                        "ab*": ["./longest-prefix.ts"]
                    }
                }
            }"#,
        )
        .expect("write wildcard priority config");
        std::fs::write(
            dir.path().join("longer-total-pattern.ts"),
            "export interface WrongPattern { value: never }",
        )
        .expect("write total pattern decoy");
        let expected = dir.path().join("longest-prefix.ts");
        std::fs::write(
            &expected,
            "export interface LongestPrefix { value: string }",
        )
        .expect("write longest prefix target");

        assert_eq!(
            resolve_vue3_type_import(
                &dir.path().join("Comp.vue").to_string_lossy(),
                "ab-value-bcd",
                &Vue3TypeResolverContext::default(),
            ),
            Some(expected)
        );
    }

    #[test]
    fn vue3_tsconfig_paths_do_not_try_weaker_patterns_after_the_best_match_misses() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "*": ["./weaker-pattern.ts"],
                        "feature-*": ["./missing-first.ts", "./missing-second.ts"]
                    }
                }
            }"#,
        )
        .expect("write best pattern miss config");
        std::fs::write(
            dir.path().join("weaker-pattern.ts"),
            "export interface WeakerPattern { shouldNotResolve: never }",
        )
        .expect("write weaker pattern decoy");

        assert!(resolve_vue3_type_import(
            &dir.path().join("Comp.vue").to_string_lossy(),
            "feature-value",
            &Vue3TypeResolverContext::default(),
        )
        .is_none());
    }

    #[test]
    fn vue3_tsconfig_paths_try_targets_of_the_best_pattern_in_order() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "ordered-targets": ["./missing.ts", "./fallback.ts"]
                    }
                }
            }"#,
        )
        .expect("write ordered target config");
        let expected = dir.path().join("fallback.ts");
        std::fs::write(
            &expected,
            "export interface OrderedFallback { value: boolean }",
        )
        .expect("write ordered fallback target");

        assert_eq!(
            resolve_vue3_type_import(
                &dir.path().join("Comp.vue").to_string_lossy(),
                "ordered-targets",
                &Vue3TypeResolverContext::default(),
            ),
            Some(expected)
        );
    }

    #[test]
    fn vue3_compile_script_resolves_tsconfig_jsonc_paths_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("src").join("base")).expect("create base dir");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                // Root path mapping.
                "compilerOptions": {
                    "paths": {
                        "root-alias": ["./root.ts",],
                        "app-alias": ["${configDir}/app.ts",],
                        "@base/*": ["${configDir}/src/base/*",],
                    },
                },
                "references": [
                    { "path": "./tsconfig.app.json", },
                ],
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("tsconfig.app.json"),
            r#"{
                "extends": [
                    "./config/base.json", // inherited alias
                ],
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                /* Extended JSONC configs remain part of the metadata graph. */
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.path().join("app.ts"),
            "export type AppProps = { app?: number }",
        )
        .expect("write app type");
        std::fs::write(
            dir.path().join("src").join("base").join("types.ts"),
            "export type BaseProps = { base: boolean }",
        )
        .expect("write base type");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'root-alias'
import type { AppProps } from 'app-alias'
import type { BaseProps } from '@base/types'
defineProps<RootProps & AppProps & BaseProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("app: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("base: { type: Boolean, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("root.ts"),
            dir.path().join("app.ts"),
            dir.path().join("src").join("base").join("types.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_package_tsconfig_extends_paths_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        let scoped_config_pkg = dir
            .path()
            .join("node_modules")
            .join("@vuec")
            .join("tsconfig");
        let scoped_config_dir = scoped_config_pkg.join("configs");
        std::fs::create_dir_all(&scoped_config_dir).expect("create scoped config package");
        std::fs::write(
            scoped_config_pkg.join("package.json"),
            r#"{"tsconfig":".\\configs\\base.json"}"#,
        )
        .expect("write scoped config package manifest");
        std::fs::write(
            scoped_config_dir.join("base.json"),
            r#"{
                // Package config entries may be JSONC.
                "compilerOptions": {
                    "paths": {
                        "pkg-root": ["${configDir}/root.ts",],
                    },
                },
            }"#,
        )
        .expect("write scoped package config");

        let preset_pkg = dir.path().join("node_modules").join("vuec-tsconfig-preset");
        std::fs::create_dir_all(&preset_pkg).expect("create preset package");
        std::fs::write(
            preset_pkg.join("shared.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "pkg-root": ["${configDir}/root.ts"],
                        "pkg-shared": ["${configDir}/shared.ts"],
                        "local-alias": ["${configDir}/local.ts"]
                    }
                }
            }"#,
        )
        .expect("write preset subpath config");

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": ["@vuec/tsconfig", "vuec-tsconfig-preset/shared"]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.path().join("shared.ts"),
            "export type SharedProps = { shared?: number }",
        )
        .expect("write shared type");
        std::fs::write(
            dir.path().join("local.ts"),
            "export type LocalProps = { local: boolean }",
        )
        .expect("write local type");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'pkg-root'
import type { SharedProps } from 'pkg-shared'
import type { LocalProps } from 'local-alias'
defineProps<RootProps & SharedProps & LocalProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("shared: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("local: { type: Boolean, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("root.ts"),
            dir.path().join("shared.ts"),
            dir.path().join("local.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_tsconfig_jsonc_preserves_string_literal_contents() {
        let value = vue3_parse_tsconfig_jsonc(
            r#"{
                "compilerOptions": {
                    "baseUrl": "./src,not-trailing",
                    "paths": {
                        "url/*": [
                            "./literal//slash/*",
                            "./literal/*block*/segment/*",
                        ],
                    },
                },
            }"#,
        )
        .expect("parse jsonc tsconfig");
        let compiler_options = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
            .expect("compiler options");
        assert_eq!(
            compiler_options
                .get("baseUrl")
                .and_then(serde_json::Value::as_str),
            Some("./src,not-trailing")
        );
        let targets = compiler_options
            .get("paths")
            .and_then(|paths| paths.get("url/*"))
            .and_then(serde_json::Value::as_array)
            .expect("paths target");
        let targets = targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            targets,
            vec!["./literal//slash/*", "./literal/*block*/segment/*"]
        );
    }

    #[test]
    fn vue3_compile_script_resolves_relative_re_exported_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export type LeafProps = { leaf?: number }",
        )
        .expect("write leaf type");
        std::fs::write(
            dir.path().join("events.ts"),
            "export type LeafEmits = { (e: 'save'): void }",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("bar.ts"),
            "export { LeafProps as BarProps } from './leaf'\nexport * from './events'",
        )
        .expect("write bar type");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export { BarProps as Props } from './bar'\nexport { LeafEmits as Emits } from './bar'\nexport * from './model'",
        )
        .expect("write foo type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props, Emits, ModelValue } from './foo'
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("leaf: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["foo.ts", "bar.ts", "leaf.ts", "events.ts", "model.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_default_type_imports_and_re_exports() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("direct_base.ts"),
            "export interface DirectBase { inherited?: string }",
        )
        .expect("write direct base type");
        std::fs::write(
            dir.path().join("direct.ts"),
            "import type { DirectBase } from './direct_base'\nexport default interface DirectProps extends DirectBase { direct?: boolean }",
        )
        .expect("write direct type");
        std::fs::write(
            dir.path().join("alias.ts"),
            "type AliasProps = { alias: string }; export default AliasProps",
        )
        .expect("write alias type");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export default interface LeafProps { leaf: string }",
        )
        .expect("write leaf type");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { default } from './leaf'",
        )
        .expect("write default facade");
        std::fs::write(
            dir.path().join("named.ts"),
            "export interface NamedProps { named: number }",
        )
        .expect("write named type");
        std::fs::write(
            dir.path().join("default_named.ts"),
            "export { NamedProps as default } from './named'",
        )
        .expect("write named default facade");
        std::fs::write(
            dir.path().join("renamed.ts"),
            "export { default as RenamedProps } from './alias'",
        )
        .expect("write renamed default facade");
        std::fs::write(
            dir.path().join("events.ts"),
            "type Events = { (e: 'save'): void }; export default Events",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("model.ts"),
            "type ModelValue = boolean | string; export default ModelValue",
        )
        .expect("write model type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import DirectProps from './direct'
import AliasProps from './alias'
import FacadeProps from './facade'
import NamedDefaultProps from './default_named'
import { RenamedProps } from './renamed'
import Events from './events'
import ModelValue from './model'
const props = defineProps<DirectProps & AliasProps & FacadeProps & NamedDefaultProps & RenamedProps>()
const emit = defineEmits<Events>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("direct: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("inherited: { type: String, required: false }"));
        assert!(script
            .content
            .contains("alias: { type: String, required: true }"));
        assert!(script
            .content
            .contains("leaf: { type: String, required: true }"));
        assert!(script
            .content
            .contains("named: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "direct_base.ts",
            "direct.ts",
            "alias.ts",
            "leaf.ts",
            "facade.ts",
            "named.ts",
            "default_named.ts",
            "renamed.ts",
            "events.ts",
            "model.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    fn write_external_type_re_export_chain(
        root: &std::path::Path,
        file_count: usize,
    ) -> std::path::PathBuf {
        assert!(file_count > 0);
        std::fs::create_dir_all(root).expect("create type chain directory");
        for index in 0..file_count {
            let source = if index + 1 == file_count {
                "export interface Leaf { value: string }".to_string()
            } else {
                format!("export {{ Leaf }} from './type_{}.ts'", index + 1)
            };
            std::fs::write(root.join(format!("type_{index}.ts")), source)
                .expect("write type chain file");
        }
        root.join("type_0.ts")
    }

    fn vue3_type_resolver_with_external_limits(
        limits: Vue3ExternalTypeLoadLimits,
    ) -> Vue3TypeResolverContext {
        Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
            ..Vue3TypeResolverContext::default()
        }
    }

    #[test]
    fn vue3_external_type_loader_bounds_active_import_depth() {
        let dir = tempfile::tempdir().expect("temp dir");
        let accepted = write_external_type_re_export_chain(
            &dir.path().join("accepted"),
            VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES,
        );
        let rejected = write_external_type_re_export_chain(
            &dir.path().join("rejected"),
            VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES + 1,
        );
        let resolver = Vue3TypeResolverContext::default();

        let mut seen = BTreeSet::new();
        let accepted_context =
            vue3_external_type_context_from_path(&accepted, &mut seen, &resolver)
                .expect("load chain at active file limit");
        assert!(accepted_context.declared_types.contains_key("Leaf"));
        assert!(seen.is_empty());

        let rejected_context =
            vue3_external_type_context_from_path(&rejected, &mut seen, &resolver)
                .expect("load bounded type chain prefix");
        assert!(!rejected_context.declared_types.contains_key("Leaf"));
        assert!(seen.is_empty());

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { Leaf } from './rejected/type_0'
defineProps<Leaf>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(
            script.errors.iter().any(|error| error
                .contains("Unresolvable type reference or unsupported built-in utility type")),
            "{:?}",
            script.errors
        );
    }

    #[test]
    fn vue3_external_type_loader_uses_canonical_path_identity() {
        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).expect("create nested directory");
        let source_path = dir.path().join("types.ts");
        std::fs::write(&source_path, "export interface Props { value: string }")
            .expect("write type file");
        let alias_path = nested.join("..").join("types.ts");
        let identity = vue3_external_type_path_identity(&source_path);
        let mut seen = BTreeSet::from([identity.clone()]);

        assert!(vue3_external_type_context_from_path(
            &alias_path,
            &mut seen,
            &Vue3TypeResolverContext::default(),
        )
        .is_none());
        assert_eq!(seen, BTreeSet::from([identity]));
    }

    #[cfg(unix)]
    #[test]
    fn vue3_external_type_caches_preserve_native_unix_paths() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let first_path = dir
            .path()
            .join(std::ffi::OsString::from_vec(b"type-\x80.ts".to_vec()));
        let second_path = dir
            .path()
            .join(std::ffi::OsString::from_vec(b"type-\x81.ts".to_vec()));
        std::fs::write(
            &first_path,
            "export interface FirstNativePath { value: string }",
        )
        .expect("write first native path");
        std::fs::write(
            &second_path,
            "export interface SecondNativePath { value: string }",
        )
        .expect("write second native path");
        assert_eq!(
            first_path.to_string_lossy(),
            second_path.to_string_lossy()
        );
        assert_ne!(
            vue3_external_type_path_identity(&first_path),
            vue3_external_type_path_identity(&second_path)
        );

        let resolver = Vue3TypeResolverContext::default();
        let first_source = vue3_external_type_source_from_path(&first_path, &resolver)
            .expect("load first native source");
        let second_source = vue3_external_type_source_from_path(&second_path, &resolver)
            .expect("load second native source");
        assert!(first_source.source.contains("FirstNativePath"));
        assert!(second_source.source.contains("SecondNativePath"));
        assert!(!std::sync::Arc::ptr_eq(&first_source, &second_source));

        let first_context = vue3_external_type_context_from_path(
            &first_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load first native context");
        let second_context = vue3_external_type_context_from_path(
            &second_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load second native context");
        assert!(first_context
            .declared_types
            .contains_key("FirstNativePath"));
        assert!(second_context
            .declared_types
            .contains_key("SecondNativePath"));
        assert!(!std::sync::Arc::ptr_eq(&first_context, &second_context));

        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 2);
        assert_eq!(stats.source_cache_hits, 2);
        assert_eq!(stats.context_builds, 2);
        assert_eq!(stats.context_cache_hits, 0);
    }

    #[test]
    fn vue3_external_type_loader_checks_active_paths_before_warm_cache() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_path = dir.path().join("types.ts");
        std::fs::write(&source_path, "export interface Props { value: string }")
            .expect("write type file");
        let resolver = Vue3TypeResolverContext::default();
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .is_some());

        let identity = vue3_external_type_path_identity(&source_path);
        let mut cyclic_seen = BTreeSet::from([identity.clone()]);
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut cyclic_seen,
            &resolver,
        )
        .is_none());
        assert_eq!(cyclic_seen, BTreeSet::from([identity]));

        let mut depth_limited_seen = (0..VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES)
            .map(|index| PathBuf::from(format!("active-{index}")))
            .collect::<BTreeSet<_>>();
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut depth_limited_seen,
            &resolver,
        )
        .is_none());
        assert_eq!(
            depth_limited_seen.len(),
            VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES
        );
        assert_eq!(resolver.external_type_session.stats().context_cache_hits, 0);
    }

    #[cfg(unix)]
    #[test]
    fn vue3_external_type_loader_resolves_symlink_identity() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_path = dir.path().join("types.ts");
        let alias_path = dir.path().join("alias.ts");
        std::fs::write(&source_path, "export interface Props { value: string }")
            .expect("write type file");
        std::os::unix::fs::symlink(&source_path, &alias_path).expect("create type file symlink");
        let identity = vue3_external_type_path_identity(&source_path);
        let mut seen = BTreeSet::from([identity.clone()]);

        assert!(vue3_external_type_context_from_path(
            &alias_path,
            &mut seen,
            &Vue3TypeResolverContext::default(),
        )
        .is_none());
        assert_eq!(seen, BTreeSet::from([identity]));
    }

    #[cfg(unix)]
    #[test]
    fn vue3_external_type_context_cache_preserves_lexical_import_base() {
        let dir = tempfile::tempdir().expect("temp dir");
        let shared_dir = dir.path().join("shared");
        let first_dir = dir.path().join("first");
        let second_dir = dir.path().join("second");
        for path in [&shared_dir, &first_dir, &second_dir] {
            std::fs::create_dir_all(path).expect("create type directory");
        }
        let shared = shared_dir.join("shared.ts");
        std::fs::write(&shared, "export { Marker } from './dep'")
            .expect("write shared type file");
        let first_alias = first_dir.join("shared.ts");
        let second_alias = second_dir.join("shared.ts");
        std::os::unix::fs::symlink(&shared, &first_alias).expect("create first type symlink");
        std::os::unix::fs::symlink(&shared, &second_alias).expect("create second type symlink");
        let first_dep = first_dir.join("dep.ts");
        let second_dep = second_dir.join("dep.ts");
        std::fs::write(&first_dep, "export interface Marker { first: string }")
            .expect("write first dependency");
        std::fs::write(&second_dep, "export interface Marker { second: string }")
            .expect("write second dependency");
        let resolver = Vue3TypeResolverContext::default();

        let first = vue3_external_type_context_from_path(
            &first_alias,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load first symlink context");
        let second = vue3_external_type_context_from_path(
            &second_alias,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load second symlink context");

        assert_eq!(
            first.type_sources.get("Marker"),
            Some(&normalize_path_string(&first_dep))
        );
        assert_eq!(
            second.type_sources.get("Marker"),
            Some(&normalize_path_string(&second_dep))
        );
    }

    #[cfg(unix)]
    #[test]
    fn vue3_external_type_source_cache_preserves_declaration_mode() {
        let dir = tempfile::tempdir().expect("temp dir");
        let declaration = dir.path().join("types.d.ts");
        let regular_alias = dir.path().join("types.ts");
        std::fs::write(&declaration, "export interface Props { value: string }")
            .expect("write declaration file");
        std::os::unix::fs::symlink(&declaration, &regular_alias)
            .expect("create regular TypeScript symlink");
        let resolver = Vue3TypeResolverContext::default();

        let regular = vue3_external_type_source_from_path(&regular_alias, &resolver)
            .expect("load regular TypeScript alias");
        let definition = vue3_external_type_source_from_path(&declaration, &resolver)
            .expect("load TypeScript declaration");
        assert!(!regular.source_type.is_typescript_definition());
        assert!(definition.source_type.is_typescript_definition());
        assert_eq!(resolver.external_type_session.stats().import_files_read, 2);

        assert!(vue3_external_type_source_from_path(&regular_alias, &resolver).is_some());
        assert!(vue3_external_type_source_from_path(&declaration, &resolver).is_some());
        assert_eq!(resolver.external_type_session.stats().source_cache_hits, 2);
    }

    #[test]
    fn vue3_external_type_loader_caches_diamond_import_contexts() {
        let dir = tempfile::tempdir().expect("temp dir");
        let leaf = dir.path().join("leaf.ts");
        let root = dir.path().join("root.ts");
        std::fs::write(&leaf, "export interface Leaf { value: string }")
            .expect("write leaf type");
        std::fs::write(
            &root,
            concat!(
                "import { Leaf as Left } from './leaf'\n",
                "import { Leaf as Right } from './leaf'\n",
                "export interface Root { left: Left; right: Right }",
            ),
        )
        .expect("write root type");
        let resolver = Vue3TypeResolverContext::default();
        let context =
            vue3_external_type_context_from_path(&root, &mut BTreeSet::new(), &resolver)
                .expect("load diamond import root");

        assert!(context.declared_types.contains_key("Root"));
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 2);
        assert_eq!(
            stats.import_bytes,
            std::fs::metadata(&root).expect("root metadata").len() as usize
                + std::fs::metadata(&leaf).expect("leaf metadata").len() as usize
        );
        assert_eq!(stats.source_cache_hits, 0);
        assert_eq!(stats.context_lookups, 3);
        assert_eq!(stats.context_builds, 2);
        assert_eq!(stats.context_cache_hits, 1);
        assert!(stats.context_build_weight > stats.import_bytes);
        assert!(stats.cached_context_weight > 0);
    }

    #[test]
    fn vue3_external_type_loader_enforces_shared_file_budgets() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cached = dir.path().join("cached.ts");
        let invalid_utf8 = dir.path().join("invalid.ts");
        let oversized = dir.path().join("oversized.ts");
        let total_overflow = dir.path().join("total.ts");
        let file_overflow = dir.path().join("file.ts");
        std::fs::write(&cached, "export {}").expect("write cached type");
        std::fs::write(&invalid_utf8, vec![0xff; 10]).expect("write invalid UTF-8 type");
        std::fs::write(&oversized, "x".repeat(17)).expect("write oversized type");
        std::fs::write(&total_overflow, "export const x=1").expect("write total overflow type");
        std::fs::write(&file_overflow, "export {}").expect("write file overflow type");
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 4,
            max_file_bytes: 16,
            max_import_bytes: 20,
            max_context_builds: 8,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let cloned = resolver.clone();

        assert!(vue3_external_type_source_from_path(&cached, &resolver).is_some());
        assert!(vue3_external_type_source_from_path(&cached, &cloned).is_some());
        assert!(vue3_external_type_source_from_path(&invalid_utf8, &resolver).is_none());
        assert!(vue3_external_type_source_from_path(&oversized, &resolver).is_none());
        assert!(vue3_external_type_source_from_path(&total_overflow, &resolver).is_none());
        assert!(vue3_external_type_source_from_path(&file_overflow, &resolver).is_none());
        assert_eq!(
            resolver.external_type_session.stats(),
            Vue3ExternalTypeLoadStats {
                import_files_read: 4,
                global_files_read: 0,
                import_bytes: "export {}".len() + 10,
                global_bytes: 0,
                source_cache_hits: 1,
                context_lookups: 0,
                context_builds: 0,
                context_build_weight: 0,
                context_cache_hits: 0,
                cached_context_weight: 0,
                resolution_lookups: 0,
                resolution_cache_hits: 0,
                cached_resolution_weight: 0,
                metadata_files_read: 0,
                metadata_bytes: 0,
                metadata_source_cache_hits: 0,
                metadata_parse_cache_hits: 0,
                metadata_fanout_entries: 0,
                metadata_resolution_path_probes: 0,
                tsconfig_nodes: 0,
                tsconfig_discovery_entries: 0,
                tsconfig_discovery_files: 0,
                tsconfig_glob_match_steps: 0,
                ancestor_search_entries: 0,
                ancestor_search_weight: 0,
            }
        );
    }

    #[test]
    fn vue3_external_type_loader_honors_exact_byte_boundaries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let exact_file = dir.path().join("exact.ts");
        let oversized_file = dir.path().join("oversized.ts");
        let exact_total = dir.path().join("total.ts");
        let total_overflow = dir.path().join("overflow.ts");
        std::fs::write(&exact_file, "type").expect("write exact file");
        std::fs::write(&oversized_file, "types").expect("write oversized file");
        std::fs::write(&exact_total, "x").expect("write exact total file");
        std::fs::write(&total_overflow, "x").expect("write total overflow file");
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 8,
            max_file_bytes: 4,
            max_import_bytes: 5,
            max_context_builds: 8,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(vue3_external_type_source_from_path(&exact_file, &resolver).is_some());
        assert!(vue3_external_type_source_from_path(&oversized_file, &resolver).is_none());
        assert!(vue3_external_type_source_from_path(&exact_total, &resolver).is_some());
        assert!(vue3_external_type_source_from_path(&total_overflow, &resolver).is_none());
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 4);
        assert_eq!(stats.import_bytes, 5);
    }

    #[test]
    fn vue3_external_type_loader_bounds_context_builds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = write_external_type_re_export_chain(dir.path(), 3);
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 8,
            max_file_bytes: 1024,
            max_import_bytes: 4096,
            max_context_builds: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let context =
            vue3_external_type_context_from_path(&root, &mut BTreeSet::new(), &resolver)
                .expect("load bounded context prefix");

        assert!(!context.declared_types.contains_key("Leaf"));
        assert_eq!(resolver.external_type_session.stats().context_builds, 2);
    }

    #[test]
    fn vue3_external_type_loader_enforces_context_lookup_budget_on_cache_hits() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_path = dir.path().join("types.ts");
        std::fs::write(&source_path, "export interface Props { value: string }")
            .expect("write type file");
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_context_lookups: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .is_some());
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .is_none());
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.context_lookups, 1);
        assert_eq!(stats.context_builds, 1);
        assert_eq!(stats.context_cache_hits, 0);
    }

    #[test]
    fn vue3_external_type_loader_bounds_uncached_context_build_weight() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_path = dir.path().join("types.ts");
        let source = "export interface SecretContextMarker { value: string }";
        std::fs::write(&source_path, source).expect("write type file");
        let measuring_resolver = Vue3TypeResolverContext::default();
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &measuring_resolver,
        )
        .is_some());
        let exact_build_weight = measuring_resolver
            .external_type_session
            .stats()
            .context_build_weight;
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_context_build_weight: exact_build_weight,
            max_context_cache_weight: 0,
            max_context_cache_entry_weight: 0,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        let context = vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load context at exact build weight limit");
        assert!(context
            .declared_types
            .contains_key("SecretContextMarker"));
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &resolver,
        )
        .is_none());
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.import_files_read, 1);
        assert_eq!(stats.source_cache_hits, 0);
        assert_eq!(stats.context_lookups, 2);
        assert_eq!(stats.context_builds, 1);
        assert_eq!(stats.context_build_weight, exact_build_weight);
        assert_eq!(stats.context_cache_hits, 0);
        assert_eq!(stats.cached_context_weight, 0);
        assert!(!format!("{resolver:?}").contains("SecretContextMarker"));

        let rejecting_resolver =
            vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
                max_context_build_weight: exact_build_weight - 1,
                ..Vue3ExternalTypeLoadLimits::default()
            });
        assert!(vue3_external_type_context_from_path(
            &source_path,
            &mut BTreeSet::new(),
            &rejecting_resolver,
        )
        .is_none());
        assert_eq!(
            rejecting_resolver
                .external_type_session
                .stats()
                .context_build_weight,
            exact_build_weight - 1
        );
    }

    #[test]
    fn vue3_external_type_loader_deduplicates_global_paths_before_parsing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).expect("create nested directory");
        let global = dir.path().join("global.d.ts");
        std::fs::write(&global, "declare interface GlobalProps { value: string }")
            .expect("write global type file");
        let alias = nested.join("..").join("global.d.ts");
        let files = vec![
            global.to_string_lossy().to_string(),
            alias.to_string_lossy().to_string(),
            global.to_string_lossy().to_string(),
        ];
        let resolver = Vue3TypeResolverContext::default();

        let context = vue3_global_type_context(
            &dir.path().join("Comp.vue").to_string_lossy(),
            &files,
            &resolver,
        );
        assert!(context.declared_types.contains_key("GlobalProps"));
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.global_files_read, 1);
        assert_eq!(stats.source_cache_hits, 0);
        assert_eq!(stats.context_lookups, 1);
        assert_eq!(stats.context_builds, 1);
    }

    #[test]
    fn vue3_external_type_loader_reserves_import_budget_from_globals() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first_global = dir.path().join("global-one.d.ts");
        let second_global = dir.path().join("global-two.d.ts");
        let imported = dir.path().join("props.ts");
        std::fs::write(&first_global, "declare interface GlobalOne {}")
            .expect("write first global type");
        std::fs::write(&second_global, "declare interface GlobalTwo {}")
            .expect("write second global type");
        std::fs::write(&imported, "export interface Props { value: string }")
            .expect("write imported type");
        let resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_import_files: 1,
            max_global_files: 1,
            max_import_bytes: 1024,
            max_global_bytes: 1024,
            ..Vue3ExternalTypeLoadLimits::default()
        });

        assert!(vue3_global_type_context_from_path(
            &first_global,
            &Vue27TypeContext::default(),
            &resolver,
        )
        .is_some());
        assert!(vue3_global_type_context_from_path(
            &second_global,
            &Vue27TypeContext::default(),
            &resolver,
        )
        .is_none());
        let imported_context =
            vue3_external_type_context_from_path(&imported, &mut BTreeSet::new(), &resolver)
                .expect("load direct import after global budget exhaustion");
        assert!(imported_context.declared_types.contains_key("Props"));
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.global_files_read, 1);
        assert_eq!(stats.import_files_read, 1);
    }
