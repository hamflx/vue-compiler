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
    fn vue3_tsconfig_exclude_filters_only_include_discovery() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types = dir.path().join("types");
        for directory in [
            types.join("nested"),
            types.join("excluded").join("deep"),
            types.join("template-excluded"),
        ] {
            std::fs::create_dir_all(directory).expect("create excluded types fixture");
        }
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": ["./types/explicit.d.ts"],
                "include": ["./types/**/*.d.ts"],
                "exclude": [
                    "./types/excluded",
                    "./types/**/*.test.d.ts",
                    ".\\types\\exact.d.ts",
                    "${configDir}/types/template-excluded",
                    "./types/explicit.d.ts"
                ],
                "compilerOptions": { "types": [] }
            }"#,
        )
        .expect("write exclude config");
        let explicit = types.join("explicit.d.ts");
        let kept = types.join("nested").join("kept.d.ts");
        for (path, source) in [
            (
                &explicit,
                "declare interface ExplicitProps { explicitValue: string }",
            ),
            (
                &kept,
                "declare interface KeptProps { keptValue?: number }",
            ),
            (
                &types.join("excluded").join("deep").join("hidden.d.ts"),
                "declare interface HiddenProps { hiddenValue: boolean }",
            ),
            (
                &types.join("nested").join("hidden.test.d.ts"),
                "declare interface TestProps { testValue: boolean }",
            ),
            (
                &types.join("exact.d.ts"),
                "declare interface ExactProps { exactValue: boolean }",
            ),
            (
                &types.join("template-excluded").join("hidden.d.ts"),
                "declare interface TemplateHiddenProps { templateHidden: boolean }",
            ),
        ] {
            std::fs::write(path, source).expect("write exclude declaration fixture");
        }

        let filename = dir.path().join("src").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        assert_eq!(
            vue3_tsconfig_global_type_files(&filename_text, &type_resolver)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            [explicit.clone(), kept.clone()].into_iter().collect()
        );

        let source = r#"<script setup lang="ts">
defineProps<ExplicitProps & KeptProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("explicitValue: { type: String, required: true }"));
        assert!(script
            .content
            .contains("keptValue: { type: Number, required: false }"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [explicit, kept]
                .into_iter()
                .map(|path| normalize_path_string(&path))
                .collect()
        );
    }

    #[test]
    fn vue3_tsconfig_file_specs_follow_extends_replacement_and_origins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let configs = dir.path().join("configs");
        let project = dir.path().join("project");
        let globals = dir.path().join("globals");
        let shared = dir.path().join("shared");
        let reference = dir.path().join("reference");
        for directory in [
            configs.clone(),
            project.join("direct"),
            globals.clone(),
            shared.join("first-excluded"),
            shared.join("second-excluded"),
            shared.join("root-excluded"),
            reference.clone(),
        ] {
            std::fs::create_dir_all(directory).expect("create extends file-spec fixture");
        }
        std::fs::write(
            configs.join("first.json"),
            r#"{
                "files": ["../globals/first.d.ts"],
                "include": ["../shared/**/*.d.ts"],
                "exclude": ["../shared/first-excluded"],
                "references": [{ "path": "../reference" }]
            }"#,
        )
        .expect("write first base config");
        std::fs::write(
            configs.join("second.json"),
            r#"{
                "files": ["../globals/second.d.ts"],
                "exclude": ["../shared/second-excluded"]
            }"#,
        )
        .expect("write second base config");
        std::fs::write(
            reference.join("tsconfig.json"),
            r#"{"files":["./leaked.d.ts"],"compilerOptions":{"types":[]}}"#,
        )
        .expect("write non-inherited reference config");

        let first_file = globals.join("first.d.ts");
        let second_file = globals.join("second.d.ts");
        let kept = shared.join("kept.d.ts");
        let first_excluded = shared.join("first-excluded").join("value.d.ts");
        let second_excluded = shared.join("second-excluded").join("value.d.ts");
        let root_excluded = shared.join("root-excluded").join("value.d.ts");
        let direct = project.join("direct").join("value.d.ts");
        let leaked_reference = reference.join("leaked.d.ts");
        for path in [
            &first_file,
            &second_file,
            &kept,
            &first_excluded,
            &second_excluded,
            &root_excluded,
            &direct,
            &leaked_reference,
        ] {
            std::fs::write(path, "declare interface Fixture {}").expect("write file-spec fixture");
        }
        let filename = project.join("src").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let discover = || {
            vue3_tsconfig_global_type_files(
                &filename_text,
                &vue3_type_resolver_context_for_filename(&filename_text),
            )
            .into_iter()
            .collect::<BTreeSet<_>>()
        };

        std::fs::write(
            project.join("tsconfig.json"),
            r#"{
                "extends": ["../configs/first.json", "../configs/second.json"],
                "compilerOptions": { "types": [] }
            }"#,
        )
        .expect("write multiple extends config");
        assert_eq!(
            discover(),
            [second_file.clone(), kept.clone(), first_excluded.clone(), root_excluded.clone()]
                .into_iter()
                .collect()
        );

        std::fs::write(
            project.join("tsconfig.json"),
            r#"{
                "extends": ["../configs/first.json", "../configs/second.json"],
                "files": [],
                "include": ["./direct/**/*.d.ts"],
                "exclude": [],
                "compilerOptions": { "types": [] }
            }"#,
        )
        .expect("write direct override config");
        assert_eq!(discover(), [direct].into_iter().collect());

        std::fs::write(
            project.join("tsconfig.json"),
            r#"{
                "extends": "../configs/first.json",
                "exclude": ["../shared/root-excluded"],
                "compilerOptions": { "types": [] }
            }"#,
        )
        .expect("write inherited include with direct exclude config");
        assert_eq!(
            discover(),
            [first_file, kept, first_excluded, second_excluded]
                .into_iter()
                .collect()
        );
        assert!(!discover().contains(&leaked_reference));
    }

    #[test]
    fn vue3_tsconfig_global_files_use_only_the_nearest_project_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(nested.join("src")).expect("create nested project");
        let outer = dir.path().join("outer.d.ts");
        let inner = nested.join("inner.d.ts");
        std::fs::write(&outer, "declare interface OuterProps {}")
            .expect("write outer declaration");
        std::fs::write(&inner, "declare interface InnerProps {}")
            .expect("write inner declaration");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"files":["./outer.d.ts"],"compilerOptions":{"types":[]}}"#,
        )
        .expect("write outer config");
        std::fs::write(
            nested.join("tsconfig.json"),
            r#"{"files":["./inner.d.ts"],"compilerOptions":{"types":[]}}"#,
        )
        .expect("write nearest config");
        let filename = nested.join("src").join("Comp.vue");
        let filename_text = filename.to_string_lossy();

        assert_eq!(
            vue3_tsconfig_global_type_files(
                &filename_text,
                &vue3_type_resolver_context_for_filename(&filename_text),
            ),
            vec![inner]
        );
    }

    #[test]
    fn vue3_tsconfig_default_include_honors_output_dirs_and_explicit_empty_specs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let configs = dir.path().join("configs");
        let project = dir.path().join("project");
        for directory in [
            configs.clone(),
            project.join("src"),
            project.join("nested"),
            project.join("dist"),
            project.join("declarations"),
            project.join("node_modules").join("package"),
            project.join("bower_components").join("package"),
            project.join("jspm_packages").join("package"),
            project.join(".hidden"),
        ] {
            std::fs::create_dir_all(directory).expect("create default include fixture");
        }
        std::fs::write(
            configs.join("base.json"),
            r#"{
                "compilerOptions": {
                    "types": [],
                    "outDir": "../project/dist",
                    "declarationDir": "${configDir}/declarations"
                }
            }"#,
        )
        .expect("write default include base config");
        let config_path = project.join("tsconfig.json");
        std::fs::write(
            &config_path,
            r#"{"extends":"../configs/base.json"}"#,
        )
        .expect("write default include project config");
        let root = project.join("ambient.d.ts");
        let nested = project.join("nested").join("ambient.d.mts");
        let out_dir = project.join("dist").join("generated.d.ts");
        let declaration_dir = project.join("declarations").join("generated.d.ts");
        for (path, source) in [
            (
                &root,
                "declare interface DefaultIncludedProps { rootValue: string }",
            ),
            (
                &nested,
                "declare interface NestedDefaultProps { nestedValue?: number }",
            ),
            (
                &out_dir,
                "declare interface OutputDirectoryProps { outputValue: boolean }",
            ),
            (
                &declaration_dir,
                "declare interface DeclarationDirectoryProps { declarationValue: boolean }",
            ),
            (
                &project
                    .join("node_modules")
                    .join("package")
                    .join("ignored.d.ts"),
                "declare interface NodeModulesDefaultProps {}",
            ),
            (
                &project
                    .join("bower_components")
                    .join("package")
                    .join("ignored.d.ts"),
                "declare interface BowerDefaultProps {}",
            ),
            (
                &project
                    .join("jspm_packages")
                    .join("package")
                    .join("ignored.d.ts"),
                "declare interface JspmDefaultProps {}",
            ),
            (
                &project.join(".hidden").join("ignored.d.ts"),
                "declare interface HiddenDefaultProps {}",
            ),
        ] {
            std::fs::write(path, source).expect("write default include declaration");
        }
        let filename = project.join("src").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let discover = || {
            vue3_tsconfig_global_type_files(
                &filename_text,
                &vue3_type_resolver_context_for_filename(&filename_text),
            )
            .into_iter()
            .collect::<BTreeSet<_>>()
        };

        assert_eq!(
            discover(),
            [root.clone(), nested.clone()].into_iter().collect()
        );
        let bounded = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_files: 2,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_tsconfig_global_type_files(&filename_text, &bounded)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            [root.clone(), nested.clone()].into_iter().collect()
        );
        assert_eq!(
            bounded
                .external_type_session
                .stats()
                .tsconfig_discovery_files,
            2
        );
        assert!(!bounded.external_type_session.metadata_is_blocked());
        let source = r#"<script setup lang="ts">
defineProps<DefaultIncludedProps & NestedDefaultProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("rootValue: { type: String, required: true }"));
        assert!(script
            .content
            .contains("nestedValue: { type: Number, required: false }"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [root.clone(), nested.clone()]
                .into_iter()
                .map(|path| normalize_path_string(&path))
                .collect()
        );

        for config in [
            r#"{"extends":"../configs/base.json","files":[]}"#,
            r#"{"extends":"../configs/base.json","include":[]}"#,
        ] {
            std::fs::write(&config_path, config).expect("write empty project file spec");
            assert!(discover().is_empty());
        }

        std::fs::write(
            &config_path,
            r#"{"extends":"../configs/base.json","exclude":[]}"#,
        )
        .expect("write explicit empty exclude");
        assert_eq!(
            discover(),
            [root, nested, out_dir, declaration_dir]
                .into_iter()
                .collect()
        );
    }

    #[cfg(windows)]
    #[test]
    fn vue3_tsconfig_discovery_matches_paths_case_insensitively_on_windows() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("project");
        for directory in [
            project.join("src"),
            project.join("dist"),
            project.join("NODE_MODULES").join("package"),
            project.join("BOWER_COMPONENTS").join("package"),
            project.join("JSPM_PACKAGES").join("package"),
        ] {
            std::fs::create_dir_all(directory).expect("create case-insensitive fixture");
        }
        let config_path = project.join("tsconfig.json");
        std::fs::write(
            &config_path,
            r#"{"compilerOptions":{"types":[],"outDir":"./DIST"}}"#,
        )
        .expect("write case-insensitive output config");
        let ambient = project.join("ambient.d.ts");
        std::fs::write(&ambient, "declare interface CaseInsensitiveProps {}")
            .expect("write included declaration");
        for path in [
            project.join("dist").join("generated.d.ts"),
            project
                .join("NODE_MODULES")
                .join("package")
                .join("ignored.d.ts"),
            project
                .join("BOWER_COMPONENTS")
                .join("package")
                .join("ignored.d.ts"),
            project
                .join("JSPM_PACKAGES")
                .join("package")
                .join("ignored.d.ts"),
        ] {
            std::fs::write(path, "declare interface IgnoredProps {}")
                .expect("write excluded declaration");
        }
        let filename = project.join("src").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let bounded = vue3_type_resolver_with_external_limits(Vue3ExternalTypeLoadLimits {
            max_tsconfig_discovery_files: 1,
            ..Vue3ExternalTypeLoadLimits::default()
        });
        assert_eq!(
            vue3_tsconfig_global_type_files(&filename_text, &bounded),
            vec![ambient.clone()]
        );
        assert_eq!(
            bounded
                .external_type_session
                .stats()
                .tsconfig_discovery_files,
            1
        );
        assert!(!bounded.external_type_session.metadata_is_blocked());

        let ignored = project.join("ignored").join("deep");
        std::fs::create_dir_all(&ignored).expect("create wildcard exclude fixture");
        std::fs::write(
            ignored.join("value.d.ts"),
            "declare interface WildcardIgnoredProps {}",
        )
        .expect("write wildcard excluded declaration");
        std::fs::write(
            &config_path,
            r#"{
                "include":["./ambient.d.ts","./ignored/**/*.d.ts"],
                "exclude":["./IGNORED/**/*.D.TS"],
                "compilerOptions":{"types":[]}
            }"#,
        )
        .expect("write case-insensitive wildcard config");
        assert_eq!(
            vue3_tsconfig_global_type_files(
                &filename_text,
                &vue3_type_resolver_context_for_filename(&filename_text),
            ),
            vec![ambient]
        );
    }

    #[test]
    fn vue3_tsconfig_filesystem_fields_accept_windows_separators_cross_platform() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src = dir.path().join("src");
        let shared = dir.path().join("shared");
        let globals = dir.path().join("globals");
        let base_type_root = dir.path().join("base-typings").join("base-root");
        let config = dir.path().join("config");
        let referenced = dir.path().join("referenced").join("project");
        for directory in [
            src.join("components"),
            shared.clone(),
            globals.join("nested"),
            base_type_root.clone(),
            config.clone(),
            referenced.join("globals"),
        ] {
            std::fs::create_dir_all(directory).expect("create tsconfig path fixture");
        }

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": ".\\config\\base.json",
                "files": [".\\globals\\root.d.ts"],
                "include": [".\\globals\\nested\\**\\*.d.ts"],
                "references": [{ "path": "referenced\\project" }],
                "compilerOptions": {
                    "baseUrl": ".\\src",
                    "paths": { "mapped": ["..\\shared\\mapped.ts"] }
                }
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            config.join("base.json"),
            r#"{"compilerOptions":{"typeRoots":["..\\base-typings"]}}"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            referenced.join("tsconfig.json"),
            r#"{"include":[".\\globals\\**\\*.d.ts"]}"#,
        )
        .expect("write referenced tsconfig");

        let base_url_type = src.join("base-url.ts");
        let mapped_type = shared.join("mapped.ts");
        let root_global = globals.join("root.d.ts");
        let included_global = globals.join("nested").join("included.d.ts");
        let base_root_global = base_type_root.join("index.d.ts");
        let referenced_global = referenced.join("globals").join("referenced.d.ts");
        for (path, source) in [
            (
                &base_url_type,
                "export interface BaseUrlProps { baseUrl: string }",
            ),
            (
                &mapped_type,
                "export interface MappedProps { mapped: number }",
            ),
            (
                &root_global,
                "declare interface RootFileProps { rootFile: boolean }",
            ),
            (
                &included_global,
                "declare interface IncludedFileProps { includedFile?: string }",
            ),
            (
                &base_root_global,
                "declare interface BaseRootProps { baseRoot: number }",
            ),
            (
                &referenced_global,
                "declare interface ReferencedProps { referenced: boolean }",
            ),
        ] {
            std::fs::write(path, source).expect("write tsconfig path type fixture");
        }

        let filename = src.join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { BaseUrlProps } from 'base-url'
import type { MappedProps } from 'mapped'
defineProps<
  BaseUrlProps & MappedProps & RootFileProps & IncludedFileProps & BaseRootProps & ReferencedProps
>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "baseUrl: { type: String, required: true }",
            "mapped: { type: Number, required: true }",
            "rootFile: { type: Boolean, required: true }",
            "includedFile: { type: String, required: false }",
            "baseRoot: { type: Number, required: true }",
            "referenced: { type: Boolean, required: true }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [
                base_url_type,
                mapped_type,
                root_global,
                included_global,
                base_root_global,
                referenced_global,
            ]
            .iter()
            .map(|path| normalize_path_string(path))
            .collect::<BTreeSet<_>>()
        );
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
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

    fn vue3_test_glob_segment_dp(pattern: &str, text: &str) -> bool {
        let pattern = pattern.chars().collect::<Vec<_>>();
        let text = text.chars().collect::<Vec<_>>();
        let mut previous = vec![false; text.len() + 1];
        previous[0] = true;
        for pattern_ch in pattern {
            let mut current = vec![false; text.len() + 1];
            if pattern_ch == '*' {
                current[0] = previous[0];
                for index in 1..=text.len() {
                    current[index] = previous[index] || current[index - 1];
                }
            } else {
                for index in 1..=text.len() {
                    current[index] = previous[index - 1]
                        && (pattern_ch == '?' || pattern_ch == text[index - 1]);
                }
            }
            previous = current;
        }
        previous[text.len()]
    }

    fn vue3_test_glob_parts_dp(pattern: &[&str], path: &[&str]) -> bool {
        let mut previous = vec![false; path.len() + 1];
        let mut current = vec![false; path.len() + 1];
        previous[0] = true;
        for pattern_part in pattern {
            current.fill(false);
            if *pattern_part == "**" {
                current[0] = previous[0];
                for path_index in 1..=path.len() {
                    current[path_index] = previous[path_index] || current[path_index - 1];
                }
            } else {
                for path_index in 1..=path.len() {
                    current[path_index] = previous[path_index - 1]
                        && vue3_test_glob_segment_dp(pattern_part, path[path_index - 1]);
                }
            }
            std::mem::swap(&mut previous, &mut current);
        }
        previous[path.len()]
    }

    fn vue3_test_glob_strings(alphabet: &[char], max_len: usize) -> Vec<String> {
        let mut values = vec![String::new()];
        let mut exact = vec![String::new()];
        for _ in 0..max_len {
            let mut next = Vec::new();
            for prefix in &exact {
                for ch in alphabet {
                    let mut value = prefix.clone();
                    value.push(*ch);
                    next.push(value);
                }
            }
            values.extend(next.iter().cloned());
            exact = next;
        }
        values
    }

    fn vue3_test_glob_sequences<'a>(choices: &[&'a str], max_len: usize) -> Vec<Vec<&'a str>> {
        let mut values = vec![Vec::new()];
        let mut exact = vec![Vec::new()];
        for _ in 0..max_len {
            let mut next = Vec::new();
            for prefix in &exact {
                for choice in choices {
                    let mut value = prefix.clone();
                    value.push(*choice);
                    next.push(value);
                }
            }
            values.extend(next.iter().cloned());
            exact = next;
        }
        values
    }

    #[test]
    fn vue3_tsconfig_glob_matching_matches_dp_semantics() {
        let patterns = vue3_test_glob_strings(&['a', 'b', '*', '?'], 4);
        let texts = vue3_test_glob_strings(&['a', 'b', '\u{00e9}'], 3);
        for pattern in &patterns {
            for text in &texts {
                assert_eq!(
                    vue3_tsconfig_glob_segment_match(pattern, text),
                    vue3_test_glob_segment_dp(pattern, text),
                    "segment pattern={pattern:?}, text={text:?}"
                );
            }
        }

        let patterns =
            vue3_test_glob_sequences(&["", "a", "b", "*", "?", "**", "a*", "*b"], 2);
        let paths = vue3_test_glob_sequences(&["", "a", "b", "ab", "\u{00e9}"], 2);
        for pattern in &patterns {
            for path in &paths {
                assert_eq!(
                    vue3_tsconfig_glob_parts_match(pattern, path),
                    vue3_test_glob_parts_dp(pattern, path),
                    "path pattern={pattern:?}, path={path:?}"
                );
            }
        }

        let paths = vue3_test_glob_sequences(&["a", "b", "x"], 4);
        for pattern in [
            &["**", "a", "**", "b"][..],
            &["a", "**", "b", "**"],
            &["**", "a", "**", "a"],
            &["**", "**", "a"],
            &["a", "**", "**", "b"],
        ] {
            for path in &paths {
                assert_eq!(
                    vue3_tsconfig_glob_parts_match(pattern, path),
                    vue3_test_glob_parts_dp(pattern, path),
                    "multi-globstar pattern={pattern:?}, path={path:?}"
                );
            }
        }

        assert!(vue3_tsconfig_glob_matches(
            r"src\**\global-?.d.ts",
            "src/nested/global-\u{00e9}.d.ts"
        ));
        assert!(!vue3_tsconfig_glob_matches(
            "src/*/global-?.d.ts",
            "src/nested/deep/global-a.d.ts"
        ));
        for (pattern, path, expected) in [
            ("", "", true),
            ("/", "/", true),
            ("a//b", "a//b", true),
            ("a/*/b", "a//b", true),
            ("a/?/b", "a//b", false),
            ("**/", "a/", true),
            ("**/", "a", false),
            ("**/a/b", "x/a/x/a/b", true),
        ] {
            assert_eq!(
                vue3_tsconfig_glob_matches(pattern, path),
                expected,
                "raw pattern={pattern:?}, path={path:?}"
            );
        }
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
        if vue3_test_non_utf8_path_operation_is_unsupported(
            "creating first non-UTF-8 directory",
            std::fs::create_dir_all(&first),
        ) {
            return;
        }
        if vue3_test_non_utf8_path_operation_is_unsupported(
            "creating second non-UTF-8 directory",
            std::fs::create_dir_all(&second),
        ) {
            return;
        }
        let first_file = first.join("first.d.ts");
        let second_file = second.join("second.d.ts");
        if vue3_test_non_utf8_path_operation_is_unsupported(
            "writing into first non-UTF-8 directory",
            std::fs::write(&first_file, "declare interface First {}"),
        ) {
            return;
        }
        if vue3_test_non_utf8_path_operation_is_unsupported(
            "writing into second non-UTF-8 directory",
            std::fs::write(&second_file, "declare interface Second {}"),
        ) {
            return;
        }
        let resolver = Vue3TypeResolverContext::default();
        let mut files = Vec::new();

        vue3_collect_global_type_files_from_dir(&types, &mut files, &resolver);

        assert_eq!(
            files.into_iter().collect::<BTreeSet<_>>(),
            [first_file, second_file].into_iter().collect()
        );
    }

    #[test]
    fn vue3_compile_script_uses_effective_tsconfig_types_and_type_roots_global_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("typings").join("chosen"))
            .expect("create chosen type root");
        std::fs::create_dir_all(dir.path().join("typings").join("@scope").join("tool"))
            .expect("create scoped type root");
        std::fs::create_dir_all(dir.path().join("typings").join("ignored"))
            .expect("create ignored type root");
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
                "files": [],
                "compilerOptions": {
                    "types": ["chosen", "@scope/tool"]
                },
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["${configDir}/typings"]
                }
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("project").join("tsconfig.json"),
            r#"{"files":[]}"#,
        )
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
defineProps<ChosenGlobalProps & DefaultTypesGlobalProps>()
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
    fn vue3_tsconfig_global_type_packages_follow_extends_option_overrides() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join("src")).expect("create project source dir");
        std::fs::create_dir_all(&second).expect("create second config dir");
        let write_package = |root: &std::path::Path, name: &str| {
            let package = root.join(name);
            std::fs::create_dir_all(&package).expect("create global type package");
            let entry = package.join("index.d.ts");
            std::fs::write(&entry, format!("declare interface {name}Global {{}}"))
                .expect("write global type package");
            entry
        };
        let _first_entry = write_package(&first.join("types"), "first");
        let inherited_entry = write_package(&first.join("types"), "second");
        let direct_entry = write_package(&first.join("types"), "direct");
        let overridden_root_entry = write_package(&project.join("types"), "second");
        let _base_default_entry = write_package(
            &first.join("node_modules").join("@types"),
            "default-origin",
        );
        let project_default_entry = write_package(
            &project.join("node_modules").join("@types"),
            "default-origin",
        );
        std::fs::write(
            first.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":["first"],"typeRoots":["./types"]}}"#,
        )
        .expect("write first config");
        std::fs::write(
            second.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":["second"]}}"#,
        )
        .expect("write second config");
        let config_path = project.join("tsconfig.json");
        let filename = project.join("src").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let discover = || {
            vue3_tsconfig_global_type_files(
                &filename_text,
                &vue3_type_resolver_context_for_filename(&filename_text),
            )
        };

        std::fs::write(
            &config_path,
            r#"{
                "extends":["../first/tsconfig.json","../second/tsconfig.json"],
                "files":[]
            }"#,
        )
        .expect("write inherited options config");
        assert_eq!(discover(), vec![inherited_entry]);

        std::fs::write(
            &config_path,
            r#"{
                "extends":["../first/tsconfig.json","../second/tsconfig.json"],
                "files":[],
                "compilerOptions":{"typeRoots":["./types"]}
            }"#,
        )
        .expect("write direct type roots config");
        assert_eq!(discover(), vec![overridden_root_entry]);

        std::fs::write(
            &config_path,
            r#"{
                "extends":["../first/tsconfig.json","../second/tsconfig.json"],
                "files":[],
                "compilerOptions":{"types":["direct"]}
            }"#,
        )
        .expect("write direct types config");
        assert_eq!(discover(), vec![direct_entry]);

        std::fs::write(
            &config_path,
            r#"{
                "extends":["../first/tsconfig.json","../second/tsconfig.json"],
                "files":[],
                "compilerOptions":{"types":[]}
            }"#,
        )
        .expect("write empty types config");
        let resolver = vue3_type_resolver_context_for_filename(&filename_text);
        assert!(vue3_tsconfig_global_type_files(&filename_text, &resolver).is_empty());
        assert_eq!(
            resolver
                .external_type_session
                .stats()
                .tsconfig_discovery_entries,
            0
        );

        std::fs::write(
            first.join("tsconfig.json"),
            r#"{"compilerOptions":{"types":["default-origin"]}}"#,
        )
        .expect("write inherited default roots config");
        std::fs::write(
            &config_path,
            r#"{"extends":"../first/tsconfig.json","files":[]}"#,
        )
        .expect("write default roots project config");
        assert_eq!(discover(), vec![project_default_entry]);
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
