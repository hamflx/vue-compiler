    #[test]
    fn vue3_compile_script_resolves_forward_type_alias_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Props = Mid & {
  own: string
}
type Mid = Base & {
  mid?: boolean
}
interface Base {
  inherited?: number
}
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("own: { type: String, required: true }"));
        assert!(script
            .content
            .contains("mid: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("inherited: { type: Number, required: false }"));
        assert_eq!(
            script.bindings.get("own").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("mid").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("inherited").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_external_forward_type_alias_props_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types_file = dir.path().join("types.ts");
        std::fs::write(
            &types_file,
            "export type Props = Base & { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write type alias props");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props } from './types'
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("local: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("ext: { type: String, required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [types_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_external_declared_return_type_extract_prop_types_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let props_file = dir.path().join("upload.ts");
        std::fs::write(
            &props_file,
            concat!(
                "import type { PropType } from 'vue'\n",
                "export interface UploadFile<T> { raw: T }\n",
                "export declare function uploadProps<T>(): {\n",
                "  fileList: { type: PropType<UploadFile<T>[]>, default: UploadFile<T>[] }\n",
                "}\n"
            ),
        )
        .expect("write upload props type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { uploadProps } from './upload'
type Props = ExtractPropTypes<ReturnType<typeof uploadProps>>
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("fileList: { type: Array, required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [props_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_local_type_shadows_imported_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("props.ts"),
            "export type Props = { imported: string }\nexport enum Kind { Imported = 'x' }",
        )
        .expect("write props type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props, Kind } from './props'
type Props = { local: number }
enum Kind { Local = 1 }
defineProps<Props>()
defineModel<Kind>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("local: { type: Number, required: true }"));
        assert!(!script.content.contains("imported: { type: String"));
        assert!(script.content.contains("\"modelValue\": { type: Number },"));
        assert!(!script
            .content
            .contains("\"modelValue\": { type: [String, Number] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_global_type_files_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            "declare interface GlobalProps { msg: string; count?: number }\ndeclare type GlobalEmits = { (e: 'save'): void }\ndeclare type GlobalModel = boolean | string",
        )
        .expect("write ambient global types");
        let module_global = dir.path().join("module-global.d.ts");
        std::fs::write(
            &module_global,
            "export {}\ndeclare global { interface AugmentedProps { flag: boolean } }",
        )
        .expect("write module global types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<GlobalProps & AugmentedProps>()
defineEmits<GlobalEmits>()
defineModel<GlobalModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![
                    global.to_string_lossy().to_string(),
                    module_global.to_string_lossy().to_string(),
                ],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("msg: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global, module_global]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_discovers_tsconfig_global_type_files_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("types").join("nested")).expect("create types dir");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::create_dir_all(dir.path().join("project")).expect("create project dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": ["./types/root.d.ts"],
                "include": ["./types/**/*.ts", "./src/**/*.vue"],
                "extends": "./config/base.json",
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                "files": ["${configDir}/types/base.d.ts"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("project").join("tsconfig.json"),
            r#"{
                "files": ["../types/ref.d.ts"]
            }"#,
        )
        .expect("write referenced tsconfig");
        std::fs::write(
            dir.path().join("types").join("root.d.ts"),
            "declare interface RootGlobalProps { root: string }",
        )
        .expect("write root global");
        std::fs::write(
            dir.path()
                .join("types")
                .join("nested")
                .join("included.d.ts"),
            "declare interface IncludedGlobalProps { included?: number }",
        )
        .expect("write included global");
        std::fs::write(
            dir.path().join("types").join("base.d.ts"),
            "declare interface BaseGlobalProps { base: boolean }",
        )
        .expect("write base global");
        std::fs::write(
            dir.path().join("types").join("ref.d.ts"),
            "declare type RefGlobalModel = boolean | string",
        )
        .expect("write referenced global");
        std::fs::write(
            dir.path().join("src").join("ignored.d.ts"),
            "declare interface IgnoredByVueInclude { ignored: string }",
        )
        .expect("write ignored global");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        let discovered = vue3_tsconfig_global_type_files(&filename_text, &type_resolver)
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        let expected_discovered = [
            dir.path().join("types").join("base.d.ts"),
            dir.path().join("types").join("root.d.ts"),
            dir.path()
                .join("types")
                .join("nested")
                .join("included.d.ts"),
            dir.path().join("types").join("ref.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(discovered, expected_discovered);

        let source = r#"<script setup lang="ts">
defineProps<RootGlobalProps & IncludedGlobalProps & BaseGlobalProps>()
defineModel<RefGlobalModel>()
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
            .contains("included: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("base: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("types").join("root.d.ts"),
            dir.path()
                .join("types")
                .join("nested")
                .join("included.d.ts"),
            dir.path().join("types").join("base.d.ts"),
            dir.path().join("types").join("ref.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .content
            .contains("ignored: { type: String, required: true }"));
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("ignored") || dep.contains('\\')));
    }

    #[test]
    fn vue3_tsconfig_include_scan_enforces_entry_file_and_depth_budgets() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types = dir.path().join("types");
        let nested = types.join("nested");
        let too_deep = nested.join("too-deep");
        std::fs::create_dir_all(&too_deep).expect("create nested type dirs");
        std::fs::write(types.join("root.d.ts"), "declare interface Root {}")
            .expect("write root type");
        std::fs::write(nested.join("nested.d.ts"), "declare interface Nested {}")
            .expect("write nested type");
        std::fs::write(
            too_deep.join("hidden.d.ts"),
            "declare interface Hidden {}",
        )
        .expect("write deep type");

        let depth_resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_depth: 1,
            max_tsconfig_discovery_entries: 32,
            max_tsconfig_discovery_files: 32,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let mut depth_limited = Vec::new();
        vue3_collect_global_type_files_from_dir(&types, &mut depth_limited, &depth_resolver);
        let depth_limited = depth_limited
            .into_iter()
            .map(|path| path.file_name().expect("file name").to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            depth_limited,
            ["nested.d.ts", "root.d.ts"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect()
        );

        let entry_resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_entries: 1,
            max_tsconfig_discovery_files: 32,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let mut entry_limited = Vec::new();
        vue3_collect_global_type_files_from_dir(&types, &mut entry_limited, &entry_resolver);
        assert!(entry_limited.is_empty(), "{entry_limited:?}");
        assert_eq!(
            entry_resolver
                .external_type_session
                .stats()
                .tsconfig_discovery_entries,
            1
        );

        let file_resolver = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_entries: 32,
            max_tsconfig_discovery_files: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        let mut file_limited = Vec::new();
        vue3_collect_global_type_files_from_dir(&types, &mut file_limited, &file_resolver);
        assert!(file_limited.is_empty(), "{file_limited:?}");
        assert_eq!(
            file_resolver
                .external_type_session
                .stats()
                .tsconfig_discovery_files,
            1
        );
    }

    #[test]
    fn vue3_tsconfig_glob_matching_handles_deep_double_star_patterns_iteratively() {
        assert!(vue3_tsconfig_glob_parts_match(
            &["src", "**", "*.d.ts"],
            &["src", "nested", "types", "global.d.ts"],
        ));
        assert!(vue3_tsconfig_glob_parts_match(
            &["src", "**", "*.d.ts"],
            &["src", "global.d.ts"],
        ));
        assert!(!vue3_tsconfig_glob_parts_match(
            &["src", "**", "*.d.ts"],
            &["src", "nested", "global.ts"],
        ));

        let mut pattern = vec!["**"; 1_024];
        pattern.push("target.d.ts");
        let path = vec!["directory"; 1_024];
        assert!(!vue3_tsconfig_glob_parts_match(&pattern, &path));
    }

    #[cfg(unix)]
    #[test]
    fn vue3_tsconfig_include_scan_does_not_follow_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("temp dir");
        let types = dir.path().join("types");
        let real = types.join("real");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&real).expect("create real type dir");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        std::fs::write(real.join("inside.d.ts"), "declare interface Inside {}")
            .expect("write inside type");
        std::fs::write(
            outside.join("outside.d.ts"),
            "declare interface Outside {}",
        )
        .expect("write outside type");
        symlink(&types, types.join("cycle")).expect("create cycle symlink");
        symlink(&outside, types.join("outside-link")).expect("create outside symlink");

        let resolver = Vue3TypeResolverContext::default();
        let mut files = Vec::new();
        vue3_collect_global_type_files_from_dir(&types, &mut files, &resolver);

        assert_eq!(files, vec![real.join("inside.d.ts")]);
    }

    #[cfg(unix)]
    #[test]
    fn vue3_tsconfig_include_scan_preserves_non_utf8_directory_identities() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let types = dir.path().join("types");
        let first = types.join(std::ffi::OsString::from_vec(vec![b'd', 0x80]));
        let second = types.join(std::ffi::OsString::from_vec(vec![b'd', 0x81]));
        std::fs::create_dir_all(&first).expect("create first non-UTF-8 directory");
        std::fs::create_dir_all(&second).expect("create second non-UTF-8 directory");
        let first_file = first.join("first.d.ts");
        let second_file = second.join("second.d.ts");
        std::fs::write(&first_file, "declare interface First {}").expect("write first type");
        std::fs::write(&second_file, "declare interface Second {}").expect("write second type");
        let resolver = Vue3TypeResolverContext::default();
        let mut files = Vec::new();

        vue3_collect_global_type_files_from_dir(&types, &mut files, &resolver);

        assert_eq!(
            files.into_iter().collect::<BTreeSet<_>>(),
            [first_file, second_file].into_iter().collect()
        );
    }

    #[test]
    fn vue3_compile_script_discovers_tsconfig_types_and_type_roots_global_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("typings").join("chosen"))
            .expect("create chosen type root");
        std::fs::create_dir_all(dir.path().join("typings").join("@scope").join("tool"))
            .expect("create scoped type root");
        std::fs::create_dir_all(dir.path().join("typings").join("ignored"))
            .expect("create ignored type root");
        std::fs::create_dir_all(dir.path().join("base-types").join("base-root"))
            .expect("create base type root");
        std::fs::create_dir_all(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted"),
        )
        .expect("create default @types root");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::create_dir_all(dir.path().join("project")).expect("create project dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": "./config/base.json",
                "compilerOptions": {
                    "types": ["chosen", "@scope/tool"],
                    "typeRoots": ["./typings"]
                },
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["${configDir}/base-types"]
                }
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(dir.path().join("project").join("tsconfig.json"), "{}")
            .expect("write referenced tsconfig");
        std::fs::write(
            dir.path().join("typings").join("chosen").join("index.d.ts"),
            "declare interface ChosenGlobalProps { chosen: string }",
        )
        .expect("write chosen global");
        std::fs::write(
            dir.path()
                .join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            "declare type ScopedGlobalModel = number | boolean",
        )
        .expect("write scoped global");
        std::fs::write(
            dir.path()
                .join("typings")
                .join("ignored")
                .join("index.d.ts"),
            "declare interface IgnoredTypeRootGlobalProps { ignored: string }",
        )
        .expect("write ignored type root");
        std::fs::write(
            dir.path()
                .join("base-types")
                .join("base-root")
                .join("index.d.ts"),
            "declare interface BaseRootGlobalProps { baseRoot?: number }",
        )
        .expect("write base root global");
        std::fs::write(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        let discovered = vue3_tsconfig_global_type_files(&filename_text, &type_resolver)
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        let expected_discovered = [
            dir.path()
                .join("base-types")
                .join("base-root")
                .join("index.d.ts"),
            dir.path().join("typings").join("chosen").join("index.d.ts"),
            dir.path()
                .join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(discovered, expected_discovered);

        let source = r#"<script setup lang="ts">
defineProps<ChosenGlobalProps & BaseRootGlobalProps & DefaultTypesGlobalProps>()
defineModel<ScopedGlobalModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("chosen: { type: String, required: true }"));
        assert!(script
            .content
            .contains("baseRoot: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("defaulted: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Number, Boolean] },"));
        assert!(!script.content.contains("ignored: { type: String"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(deps, expected_discovered);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("ignored") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_respects_empty_configured_tsconfig_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted"),
        )
        .expect("create default @types root");
        std::fs::write(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["./missing"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        let discovered = vue3_tsconfig_global_type_files(&filename_text, &type_resolver);
        assert!(discovered.is_empty(), "{:?}", discovered);

        let source = r#"<script setup lang="ts">
defineProps<DefaultTypesGlobalProps>()
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
        assert!(script.deps.is_empty(), "{:?}", script.deps);
        assert!(!script.content.contains("defaulted: { type: Boolean"));
    }

    #[test]
    fn vue3_compile_script_resolves_global_type_re_exports_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("base.ts");
        std::fs::write(&base, "export interface Base { age: number }").expect("write base type");
        let types = dir.path().join("types.ts");
        std::fs::write(&types, "export type Name = string").expect("write helper type");
        let foo = dir.path().join("foo.ts");
        std::fs::write(
            &foo,
            concat!(
                "import type { Base } from './base'\n",
                "import type { Name } from './types'\n",
                "export interface Foo extends Base { name: Name }"
            ),
        )
        .expect("write foo type");
        let bar = dir.path().join("bar.ts");
        std::fs::write(&bar, "export interface Bar { bar: boolean }").expect("write bar type");
        let baz = dir.path().join("baz.ts");
        std::fs::write(&baz, "export interface Baz { baz: string }").expect("write baz type");
        let package_dir = dir.path().join("node_modules").join("pkg");
        std::fs::create_dir_all(package_dir.join("dist")).expect("create package dir");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write package manifest");
        let package_types = package_dir.join("dist").join("index.d.ts");
        std::fs::write(
            &package_types,
            "export interface PackageType { value: string }",
        )
        .expect("write package types");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare global {\n",
                "  export type { Foo } from './foo'\n",
                "  export { Bar } from './bar'\n",
                "  export * from './baz'\n",
                "  export type { PackageType } from './node_modules/pkg'\n",
                "}\n",
                "export {}\n"
            ),
        )
        .expect("write global re-exports");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<Foo & Bar & Baz & PackageType>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("age: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("name: { type: String, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("baz: { type: String, required: true }"));
        assert!(script
            .content
            .contains("value: { type: String, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global, foo, base, types, bar, baz, package_types]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_global_declared_extract_prop_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        let global = dir.path().join("global-props.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare const globalProps: {\n",
                "  label: StringConstructor\n",
                "  enabled: { type: BooleanConstructor, required: true }\n",
                "}\n",
                "interface UploadFile<T> { raw: T }\n",
                "declare function uploadProps<T>(): {\n",
                "  fileList: { type: PropType<UploadFile<T>[]>, default: UploadFile<T>[] }\n",
                "}\n"
            ),
        )
        .expect("write global props");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<
  ExtractPropTypes<typeof globalProps> &
  Partial<import('vue').ExtractPropTypes<ReturnType<typeof uploadProps>>>
>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: false }"));
        assert!(script
            .content
            .contains("enabled: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("fileList: { type: Array, required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_global_type_files_use_imports_without_exposing_import_names() {
        let dir = tempfile::tempdir().expect("temp dir");
        let leaf = dir.path().join("leaf.ts");
        std::fs::write(&leaf, "export type ImportedValue = number").expect("write leaf type");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "import type { ImportedValue } from './leaf'\n",
                "export {}\n",
                "declare global { interface GlobalProps { imported: ImportedValue; msg: string } }"
            ),
        )
        .expect("write global types");

        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            dir.path().join("Comp.vue").to_string_lossy(),
            r#"<script setup lang="ts">defineProps<GlobalProps>()</script>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("imported: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("msg: { type: String, required: true }"));
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global.clone(), leaf]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);

        let descriptor = compiler.parse(
            dir.path().join("Imported.vue").to_string_lossy(),
            r#"<script setup lang="ts">defineProps<ImportedValue>()</script>"#,
        );
        let imported_script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(
            imported_script.errors.is_empty(),
            "{:?}",
            imported_script.errors
        );
        assert!(
            imported_script.deps.is_empty(),
            "{:?}",
            imported_script.deps
        );
        assert!(!imported_script.content.contains("imported: { type: Number"));
    }
