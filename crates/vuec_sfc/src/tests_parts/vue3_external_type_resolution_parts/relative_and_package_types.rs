    #[test]
    fn vue3_compile_script_resolves_relative_imported_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export interface Props { foo: string }",
        )
        .expect("write foo type");
        std::fs::create_dir_all(dir.path().join("bar")).expect("create bar dir");
        std::fs::write(
            dir.path().join("bar").join("index.tsx"),
            "export type ExtraProps = { count?: number }",
        )
        .expect("write bar type");
        std::fs::write(
            dir.path().join("events.d.ts"),
            "type E = { (e: 'save'): void }; export { E as Emits }",
        )
        .expect("write emits type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("unused.ts"),
            "export type Unused = { nope: string }",
        )
        .expect("write unused type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { Props } from './foo'
import { ExtraProps } from './bar'
import type { Emits } from './events'
import type { ModelValue } from './model'
import type { Unused } from './unused'
const props = defineProps<Props & ExtraProps>()
const emit = defineEmits<Emits>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            normalize_path_string(&dir.path().join("foo.ts")),
            normalize_path_string(&dir.path().join("bar").join("index.tsx")),
            normalize_path_string(&dir.path().join("events.d.ts")),
            normalize_path_string(&dir.path().join("model.ts")),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("unused") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_bare_package_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let types_pkg = node_modules.join("vuec-types-pkg");
        let types_dist = types_pkg.join("dist");
        std::fs::create_dir_all(&types_dist).expect("create types package");
        std::fs::write(
            types_pkg.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write types package manifest");
        std::fs::write(
            types_dist.join("index.d.ts"),
            "export interface Props { root: string }\nexport { ExtraProps } from './extra'\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write types package root");
        std::fs::write(
            types_dist.join("extra.d.ts"),
            "export type ExtraProps = { extra?: number }",
        )
        .expect("write types package extra");
        std::fs::write(
            types_dist.join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write types package model");

        let facade_pkg = node_modules.join("vuec-facade-pkg");
        std::fs::create_dir_all(&facade_pkg).expect("create facade package");
        std::fs::write(facade_pkg.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write facade manifest");
        std::fs::write(
            facade_pkg.join("index.d.ts"),
            "export { Props as FacadeProps } from 'vuec-types-pkg'",
        )
        .expect("write facade types");

        let exports_pkg = node_modules.join("vuec-exports-pkg");
        std::fs::create_dir_all(exports_pkg.join("types").join("feature"))
            .expect("create exports package");
        std::fs::create_dir_all(exports_pkg.join("types").join("internal"))
            .expect("create specific exports package path");
        std::fs::write(
            exports_pkg.join("package.json"),
            r#"{"exports":{".":{"types":"./types/index.d.ts","default":"./dist/index.js"},"./feature/*":{"types":"./types/feature/*.d.ts","default":"./dist/feature/*.js"},"./feature/internal/*":{"types":"./types/internal/*.d.ts","default":"./dist/internal/*.js"}}}"#,
        )
        .expect("write exports manifest");
        std::fs::write(
            exports_pkg.join("types").join("index.d.ts"),
            "export namespace Nested { export type Props = { flag: boolean } }",
        )
        .expect("write exports root types");
        std::fs::write(
            exports_pkg.join("types").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature: boolean }",
        )
        .expect("write exports feature types");
        std::fs::write(
            exports_pkg.join("types").join("internal").join("item.d.ts"),
            "export type InternalProps = { internal: boolean }",
        )
        .expect("write specific exports feature types");

        let ambient_pkg = node_modules.join("@types").join("vuec-ambient");
        std::fs::create_dir_all(&ambient_pkg).expect("create @types package");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambient: string }",
        )
        .expect("write @types package");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ExtraProps, Events } from 'vuec-types-pkg'
import type { FacadeProps } from 'vuec-facade-pkg'
import type { FeatureProps } from 'vuec-exports-pkg/feature/item'
import type { InternalProps } from 'vuec-exports-pkg/feature/internal/item'
import type { AmbientProps } from 'vuec-ambient'
import * as Exported from 'vuec-exports-pkg'
const props = defineProps<FacadeProps & ExtraProps & FeatureProps & InternalProps & AmbientProps & Exported.Nested.Props>()
const emit = defineEmits<Events>()
const model = defineModel<import('vuec-types-pkg').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("root: { type: String, required: true }"),
            "{}\ndeps: {:?}",
            script.content,
            script.deps
        );
        assert!(script
            .content
            .contains("extra: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("feature: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("internal: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("ambient: { type: String, required: true }"));
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
        let expected = [
            types_dist.join("index.d.ts"),
            types_dist.join("extra.d.ts"),
            types_dist.join("model.d.ts"),
            facade_pkg.join("index.d.ts"),
            exports_pkg.join("types").join("index.d.ts"),
            exports_pkg.join("types").join("feature").join("item.d.ts"),
            exports_pkg.join("types").join("internal").join("item.d.ts"),
            ambient_pkg.join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_dependency_packages_resolve_self_name_imports_with_their_own_mode() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let module_package = node_modules.join("@vuec").join("self-module");
        let commonjs_package = node_modules.join("vuec-self-commonjs");
        for package in [&module_package, &commonjs_package] {
            std::fs::create_dir_all(package.join("types").join("import"))
                .expect("create import type directory");
            std::fs::create_dir_all(package.join("types").join("require"))
                .expect("create require type directory");
        }
        std::fs::write(
            module_package.join("package.json"),
            r#"{
                "name":"@vuec/self-module",
                "type":"module",
                "exports":{
                    ".":{"types":"./types/index.d.ts"},
                    "./feature/*":{"types":{
                        "import":"./types/import/*.d.ts",
                        "require":"./types/require/*.d.ts"
                    }}
                }
            }"#,
        )
        .expect("write module self-reference manifest");
        std::fs::write(
            commonjs_package.join("package.json"),
            r#"{
                "name":"vuec-self-commonjs",
                "type":"commonjs",
                "exports":{
                    ".":{"types":"./types/index.d.ts"},
                    "./feature/*":{"types":{
                        "import":"./types/import/*.d.ts",
                        "require":"./types/require/*.d.ts"
                    }}
                }
            }"#,
        )
        .expect("write CommonJS self-reference manifest");
        std::fs::write(
            module_package.join("types").join("index.d.ts"),
            concat!(
                "export { FeatureProps as ModuleStaticProps } from '@vuec/self-module/feature/item'\n",
                "export type ModuleDynamicProps = import('@vuec/self-module/feature/item').DynamicProps",
            ),
        )
        .expect("write module self-reference root");
        std::fs::write(
            commonjs_package.join("types").join("index.d.ts"),
            concat!(
                "export { FeatureProps as CommonJsStaticProps } from 'vuec-self-commonjs/feature/item'\n",
                "export type CommonJsDynamicProps = import('vuec-self-commonjs/feature/item').DynamicProps",
            ),
        )
        .expect("write CommonJS self-reference root");
        std::fs::write(
            module_package
                .join("types")
                .join("import")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { moduleStatic: string }\n",
                "export interface DynamicProps { moduleDynamic: boolean }",
            ),
        )
        .expect("write module import target");
        std::fs::write(
            module_package
                .join("types")
                .join("require")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { wrongModuleStatic: never }\n",
                "export interface DynamicProps { wrongModuleDynamic: never }",
            ),
        )
        .expect("write module require decoy");
        std::fs::write(
            commonjs_package
                .join("types")
                .join("require")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { commonjsStatic: number }\n",
                "export interface DynamicProps { commonjsDynamic?: string }",
            ),
        )
        .expect("write CommonJS require target");
        std::fs::write(
            commonjs_package
                .join("types")
                .join("import")
                .join("item.d.ts"),
            concat!(
                "export interface FeatureProps { wrongCommonJsStatic: never }\n",
                "export interface DynamicProps { wrongCommonJsDynamic: never }",
            ),
        )
        .expect("write CommonJS import decoy");
        let nested_module_decoy = module_package
            .join("types")
            .join("node_modules")
            .join("@vuec")
            .join("self-module");
        let nested_commonjs_decoy = commonjs_package
            .join("types")
            .join("node_modules")
            .join("vuec-self-commonjs");
        for decoy in [&nested_module_decoy, &nested_commonjs_decoy] {
            std::fs::create_dir_all(decoy).expect("create nested same-name decoy");
            std::fs::write(
                decoy.join("package.json"),
                r#"{"exports":{"./feature/*":{"types":"./*.d.ts"}}}"#,
            )
            .expect("write nested same-name decoy manifest");
            std::fs::write(
                decoy.join("item.d.ts"),
                concat!(
                    "export interface FeatureProps { nestedDecoyStatic: never }\n",
                    "export interface DynamicProps { nestedDecoyDynamic: never }",
                ),
            )
            .expect("write nested same-name decoy target");
        }

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ModuleStaticProps, ModuleDynamicProps } from '@vuec/self-module'
import type { CommonJsStaticProps, CommonJsDynamicProps } from 'vuec-self-commonjs'
defineProps<ModuleStaticProps & ModuleDynamicProps & CommonJsStaticProps & CommonJsDynamicProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "moduleStatic: { type: String, required: true }",
            "moduleDynamic: { type: Boolean, required: true }",
            "commonjsStatic: { type: Number, required: true }",
            "commonjsDynamic: { type: String, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongModule"), "{}", script.content);
        assert!(!script.content.contains("wrongCommonJs"), "{}", script.content);
        assert!(!script.content.contains("nestedDecoy"), "{}", script.content);

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            module_package.join("types").join("index.d.ts"),
            module_package
                .join("types")
                .join("import")
                .join("item.d.ts"),
            commonjs_package.join("types").join("index.d.ts"),
            commonjs_package
                .join("types")
                .join("require")
                .join("item.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
    }

    #[test]
    fn vue3_dependency_self_name_export_exclusions_do_not_fall_through() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outer_package = dir.path().join("node_modules").join("vuec-self-blocked");
        let nested_package = dir
            .path()
            .join("node_modules")
            .join("container")
            .join("node_modules")
            .join("vuec-self-blocked");
        std::fs::create_dir_all(&outer_package).expect("create outer decoy package");
        std::fs::create_dir_all(&nested_package).expect("create nested self package");
        std::fs::write(
            outer_package.join("package.json"),
            r#"{"exports":{"./private":{"types":"./private.d.ts"}}}"#,
        )
        .expect("write outer decoy manifest");
        std::fs::write(
            outer_package.join("private.d.ts"),
            "export interface PrivateProps { leaked: string }",
        )
        .expect("write outer decoy type");
        std::fs::write(
            nested_package.join("package.json"),
            r#"{
                "name":"vuec-self-blocked",
                "exports":{
                    ".":{"types":"./index.d.ts"},
                    "./private":{"types":null,"default":"./leak.d.ts"}
                }
            }"#,
        )
        .expect("write nested self manifest");
        let importer = nested_package.join("index.d.ts");
        std::fs::write(&importer, "export {};").expect("write nested importer");
        std::fs::write(
            nested_package.join("leak.d.ts"),
            "export interface PrivateProps { selfLeak: string }",
        )
        .expect("write rejected self fallback target");
        let outside_resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            resolve_vue3_type_import(
                &dir.path().join("outside.ts").to_string_lossy(),
                "vuec-self-blocked/private",
                &outside_resolver,
            ),
            Some(outer_package.join("private.d.ts"))
        );
        let resolver = Vue3TypeResolverContext::default();

        assert!(resolve_vue3_type_import(
            &importer.to_string_lossy(),
            "vuec-self-blocked/private",
            &resolver,
        )
        .is_none());
        let stats = resolver.external_type_session.stats();
        assert_eq!(stats.metadata_files_read, 1);
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_bare_package_active_null_export_does_not_fall_through() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-null-conditional");
        std::fs::create_dir_all(&package).expect("create package");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "exports":{
                    "./private":{"types":null,"default":"./leak.d.ts"}
                }
            }"#,
        )
        .expect("write package manifest");
        std::fs::write(
            package.join("leak.d.ts"),
            "export interface PrivateProps { leaked: string }",
        )
        .expect("write rejected fallback target");
        let resolver = Vue3TypeResolverContext::default();

        assert!(resolve_vue3_type_import(
            &dir.path().join("outside.ts").to_string_lossy(),
            "vuec-null-conditional/private",
            &resolver,
        )
        .is_none());
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_project_self_name_exports_map_emitted_targets_and_resolution_modes_to_sources() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let output_dir = dir.path().join("dist");
        let declaration_dir = dir.path().join("declarations");
        for directory in [&source_dir, &output_dir, &declaration_dir] {
            std::fs::create_dir_all(directory).expect("create project directory");
        }
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-project-self",
                "type":"module",
                "exports":{
                    ".":{"types":{
                        "import":"./dist/root.js",
                        "require":"./declarations/root.d.cts"
                    }},
                    "./feature":{"types":{
                        "import":"./dist/feature.mjs",
                        "require":"./declarations/feature.d.cts"
                    }}
                }
            }"#,
        )
        .expect("write project self-reference manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "rootDir": "./src",
                    "outDir": "./dist",
                    "declarationDir": "./declarations"
                }
            }"#,
        )
        .expect("write project config");
        let targets = [
            (
                source_dir.join("root.ts"),
                "export interface ImportRootProps { importRoot: string }",
            ),
            (
                source_dir.join("root.cts"),
                "export interface RequireRootProps { requireRoot: number }",
            ),
            (
                source_dir.join("feature.mts"),
                "export interface ImportFeatureProps { importFeature: boolean }",
            ),
            (
                source_dir.join("feature.cts"),
                "export interface RequireFeatureProps { requireFeature?: string }",
            ),
        ];
        for (path, source) in &targets {
            std::fs::write(path, source).expect("write project self-reference source");
        }
        for (path, source) in [
            (
                output_dir.join("root.d.ts"),
                "export interface ImportRootProps { wrongOutputRoot: never }",
            ),
            (
                output_dir.join("feature.d.mts"),
                "export interface ImportFeatureProps { wrongOutputFeature: never }",
            ),
            (
                declaration_dir.join("root.d.cts"),
                "export interface RequireRootProps { wrongDeclarationRoot: never }",
            ),
            (
                declaration_dir.join("feature.d.cts"),
                "export interface RequireFeatureProps { wrongDeclarationFeature: never }",
            ),
        ] {
            std::fs::write(path, source).expect("write emitted self-reference decoy");
        }

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ImportRootProps } from 'vuec-project-self'
import type { ImportFeatureProps } from 'vuec-project-self/feature'
import type { RequireRootProps } from 'vuec-project-self' with { "resolution-mode": "require" }
import type { RequireFeatureProps } from 'vuec-project-self/feature' with { "resolution-mode": "require" }
defineProps<ImportRootProps & ImportFeatureProps & RequireRootProps & RequireFeatureProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "importRoot: { type: String, required: true }",
            "importFeature: { type: Boolean, required: true }",
            "requireRoot: { type: Number, required: true }",
            "requireFeature: { type: String, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongOutput"), "{}", script.content);
        assert!(
            !script.content.contains("wrongDeclaration"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            targets
                .iter()
                .map(|(path, _)| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_project_self_name_emit_paths_accept_windows_separators() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let output_dir = dir.path().join("dist");
        let declaration_dir = dir.path().join("declarations");
        for directory in [&source_dir, &output_dir, &declaration_dir] {
            std::fs::create_dir_all(directory).expect("create project directory");
        }
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-project-windows-paths",
                "exports":{
                    ".":{"types":"./dist/root.js"},
                    "./feature":{"types":"./declarations/feature.d.ts"}
                }
            }"#,
        )
        .expect("write project self-reference manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "rootDir": ".\\src",
                    "outDir": ".\\dist",
                    "declarationDir": ".\\declarations"
                }
            }"#,
        )
        .expect("write project config with Windows separators");
        let root = source_dir.join("root.ts");
        let feature = source_dir.join("feature.ts");
        std::fs::write(
            &root,
            "export interface RootProps { windowsRoot: string }",
        )
        .expect("write project root source");
        std::fs::write(
            &feature,
            "export interface FeatureProps { windowsFeature?: number }",
        )
        .expect("write project feature source");
        std::fs::write(
            output_dir.join("root.d.ts"),
            "export interface RootProps { wrongOutputRoot: never }",
        )
        .expect("write output root decoy");
        std::fs::write(
            declaration_dir.join("feature.d.ts"),
            "export interface FeatureProps { wrongDeclarationFeature: never }",
        )
        .expect("write declaration feature decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'vuec-project-windows-paths'
import type { FeatureProps } from 'vuec-project-windows-paths/feature'
defineProps<RootProps & FeatureProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "windowsRoot: { type: String, required: true }",
            "windowsFeature: { type: Number, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongOutput"), "{}", script.content);
        assert!(
            !script.content.contains("wrongDeclaration"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [root, feature]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_project_self_name_exports_require_typescript_4_7() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let dependency = dir.path().join("node_modules").join("vuec-versioned-self");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::create_dir_all(&dependency).expect("create fallback dependency");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-versioned-self",
                "exports":{
                    "./feature":{"types":"./src/local.d.ts"},
                    "./excluded":null
                }
            }"#,
        )
        .expect("write project self-reference manifest");
        let local = source_dir.join("local.d.ts");
        std::fs::write(&local, "export interface Props { local: string }")
            .expect("write local self-reference target");
        std::fs::write(
            dependency.join("package.json"),
            r#"{"types":"feature.d.ts"}"#,
        )
        .expect("write fallback dependency manifest");
        let fallback = dependency.join("feature.d.ts");
        std::fs::write(&fallback, "export interface Props { fallback: number }")
            .expect("write fallback dependency target");
        for name in ["excluded.d.ts", "missing.d.ts"] {
            std::fs::write(
                dependency.join(name),
                "export interface Props { wrongFallback: never }",
            )
            .expect("write excluded fallback decoy");
        }
        let importer = source_dir.join("index.d.ts");
        std::fs::write(&importer, "export {};").expect("write project importer");

        let legacy = Vue3TypeResolverContext {
            typescript_version: (4, 6, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        let current = Vue3TypeResolverContext {
            typescript_version: (4, 7, 0).into(),
            ..Vue3TypeResolverContext::default()
        };
        let importer = importer.to_string_lossy();

        assert_eq!(
            resolve_vue3_type_import(&importer, "vuec-versioned-self/feature", &legacy),
            Some(fallback)
        );
        assert_eq!(
            resolve_vue3_type_import(&importer, "vuec-versioned-self/feature", &current),
            Some(local)
        );
        assert!(
            resolve_vue3_type_import(&importer, "vuec-versioned-self/excluded", &current).is_none()
        );
        assert!(
            resolve_vue3_type_import(&importer, "vuec-versioned-self/missing", &current).is_none()
        );
        assert!(!legacy.external_type_session.metadata_is_blocked());
        assert!(!current.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_project_self_name_without_exports_uses_bare_package_lookup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        let dependency = dir
            .path()
            .join("node_modules")
            .join("vuec-project-no-exports");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::create_dir_all(&dependency).expect("create dependency package");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{
                "name":"vuec-project-no-exports",
                "types":"./src/local-decoy.ts"
            }"#,
        )
        .expect("write project manifest without exports");
        let local_decoy = source_dir.join("local-decoy.ts");
        std::fs::write(
            &local_decoy,
            "export interface ProjectProps { wrongLocalSelf: never }",
        )
        .expect("write local package decoy");
        std::fs::write(
            dependency.join("package.json"),
            r#"{"types":"index.d.ts"}"#,
        )
        .expect("write dependency manifest");
        let dependency_entry = dependency.join("index.d.ts");
        std::fs::write(
            &dependency_entry,
            "export interface ProjectProps { dependencyValue: string }",
        )
        .expect("write dependency entry");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ProjectProps } from 'vuec-project-no-exports'
defineProps<ProjectProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("dependencyValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongLocalSelf"), "{}", script.content);
        assert_eq!(script.deps, vec![normalize_path_string(&dependency_entry)]);
        assert!(!script.deps.contains(&normalize_path_string(&local_decoy)));
    }

    #[test]
    fn vue3_dependency_self_name_without_exports_uses_legacy_package_lookup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("vuec-self-legacy");
        std::fs::create_dir_all(&package).expect("create legacy self package");
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"vuec-self-legacy","exports":null}"#,
        )
        .expect("write legacy self manifest");
        let importer = package.join("index.d.ts");
        let leaf = package.join("leaf.d.ts");
        std::fs::write(&importer, "export {};").expect("write legacy self importer");
        std::fs::write(&leaf, "export interface LegacyProps { value: string }")
            .expect("write legacy self leaf");
        let resolver = Vue3TypeResolverContext::default();

        assert_eq!(
            resolve_vue3_type_import(
                &importer.to_string_lossy(),
                "vuec-self-legacy/leaf",
                &resolver,
            ),
            Some(leaf)
        );
        assert!(!resolver.external_type_session.metadata_is_blocked());
    }

    #[test]
    fn vue3_dependency_package_imports_resolve_modes_patterns_external_targets_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("vuec-imports-package");
        let deep = package.join("deep");
        let import_types = package.join("types").join("import");
        let require_types = package.join("types").join("require");
        let external = package
            .join("node_modules")
            .join("vuec-imports-external");
        let decoy = deep
            .join("node_modules")
            .join("vuec-imports-external");
        for directory in [&deep, &import_types, &require_types, &external, &decoy] {
            std::fs::create_dir_all(directory).expect("create package imports fixture");
        }
        std::fs::write(
            package.join("package.json"),
            r##"{
                "name":"vuec-imports-package",
                "type":"module",
                "exports":{
                    ".":{"types":"./deep/index.d.mts"},
                    "./commonjs":{"types":"./deep/commonjs.d.cts"}
                },
                "imports":{
                    "#feature/exact":{
                        "types":{
                            "import":"./types/import-exact.d.mts",
                            "require":"./types/require-exact.d.cts"
                        }
                    },
                    "#feature/*":{
                        "types":{
                            "import":"./types/import/*.d.mts",
                            "require":"./types/require/*.d.cts"
                        }
                    },
                    "#external":{"types":"vuec-imports-external"}
                }
            }"##,
        )
        .expect("write package imports manifest");
        let module_entry = deep.join("index.d.mts");
        let commonjs_entry = deep.join("commonjs.d.cts");
        std::fs::write(
            &module_entry,
            r#"
import type { ImportExact } from '#feature/exact'
import type { ImportPattern } from '#feature/item'
import type { ImportExternal } from '#external'
export interface ModuleProps extends ImportExact, ImportPattern, ImportExternal {}
"#,
        )
        .expect("write module imports entry");
        std::fs::write(
            &commonjs_entry,
            r#"
import type { RequireExact } from '#feature/exact'
import type { RequirePattern } from '#feature/item'
import type { RequireExternal } from '#external'
export interface CommonJsProps extends RequireExact, RequirePattern, RequireExternal {}
"#,
        )
        .expect("write CommonJS imports entry");
        let import_exact = package.join("types").join("import-exact.d.mts");
        let require_exact = package.join("types").join("require-exact.d.cts");
        let import_pattern = import_types.join("item.d.mts");
        let require_pattern = require_types.join("item.d.cts");
        for (path, source) in [
            (
                &import_exact,
                "export interface ImportExact { importExact: string }",
            ),
            (
                &require_exact,
                "export interface RequireExact { requireExact: number }",
            ),
            (
                &import_pattern,
                "export interface ImportPattern { importPattern: boolean }",
            ),
            (
                &require_pattern,
                "export interface RequirePattern { requirePattern: string }",
            ),
        ] {
            std::fs::write(path, source).expect("write package imports target");
        }
        std::fs::write(
            import_types.join("exact.d.mts"),
            "export interface ImportExact { wrongPatternExact: never }",
        )
        .expect("write import exact pattern decoy");
        std::fs::write(
            require_types.join("exact.d.cts"),
            "export interface RequireExact { wrongPatternExact: never }",
        )
        .expect("write require exact pattern decoy");
        std::fs::write(
            external.join("package.json"),
            r#"{
                "exports":{
                    ".":{
                        "types":{
                            "import":"./import.d.mts",
                            "require":"./require.d.cts"
                        }
                    }
                }
            }"#,
        )
        .expect("write external target manifest");
        let import_external = external.join("import.d.mts");
        let require_external = external.join("require.d.cts");
        std::fs::write(
            &import_external,
            "export interface ImportExternal { importExternal: number }",
        )
        .expect("write import external target");
        std::fs::write(
            &require_external,
            "export interface RequireExternal { requireExternal: boolean }",
        )
        .expect("write require external target");
        std::fs::write(decoy.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write deep external decoy manifest");
        std::fs::write(
            decoy.join("index.d.ts"),
            "export interface ImportExternal { wrongDeepExternal: never }\n\
             export interface RequireExternal { wrongDeepExternal: never }",
        )
        .expect("write deep external decoy");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ModuleProps } from 'vuec-imports-package'
import type { CommonJsProps } from 'vuec-imports-package/commonjs'
defineProps<ModuleProps & CommonJsProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "importExact: { type: String, required: true }",
            "importPattern: { type: Boolean, required: true }",
            "importExternal: { type: Number, required: true }",
            "requireExact: { type: Number, required: true }",
            "requirePattern: { type: String, required: true }",
            "requireExternal: { type: Boolean, required: true }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrong"), "{}", script.content);
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            module_entry,
            commonjs_entry,
            import_exact,
            require_exact,
            import_pattern,
            require_pattern,
            import_external,
            require_external,
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
    }

    #[test]
    fn vue3_project_package_imports_resolve_direct_source_targets_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "imports": {
                    "#project-props": "./src/project-props.ts",
                    "#external-props": "vuec-project-import-external"
                }
            }"##,
        )
        .expect("write project imports manifest");
        let project_props = source_dir.join("project-props.ts");
        std::fs::write(
            &project_props,
            "export interface ProjectProps { projectValue: string }",
        )
        .expect("write project imports target");
        let external_package = dir
            .path()
            .join("node_modules")
            .join("vuec-project-import-external");
        let external_decoy = source_dir
            .join("node_modules")
            .join("vuec-project-import-external");
        for package in [&external_package, &external_decoy] {
            std::fs::create_dir_all(package).expect("create external imports target");
            std::fs::write(package.join("package.json"), r#"{"types":"index.d.ts"}"#)
                .expect("write external imports manifest");
        }
        let external_props = external_package.join("index.d.ts");
        std::fs::write(
            &external_props,
            "export interface ExternalProps { externalValue: number }",
        )
        .expect("write external imports target");
        std::fs::write(
            external_decoy.join("index.d.ts"),
            "export interface ExternalProps { wrongExternalRoot: never }",
        )
        .expect("write external imports decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ProjectProps } from '#project-props'
import type { ExternalProps } from '#external-props'
defineProps<ProjectProps & ExternalProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("projectValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("externalValue: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongExternalRoot"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [project_props, external_props]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_project_tsconfig_paths_precede_package_maps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "name":"vuec-path-priority",
                "imports":{"#choice":"./src/imports-choice.ts"},
                "exports":{"./choice":{"types":"./src/self-choice.ts"}}
            }"##,
        )
        .expect("write project package maps");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r##"{
                "compilerOptions":{
                    "paths":{
                        "#choice":["./src/paths-choice.ts"],
                        "vuec-path-priority/choice":["./src/paths-self-choice.ts"]
                    }
                }
            }"##,
        )
        .expect("write project config");
        let paths_choice = source_dir.join("paths-choice.ts");
        std::fs::write(
            &paths_choice,
            "export interface ChoiceProps { pathsValue: string }",
        )
        .expect("write paths target");
        let paths_self_choice = source_dir.join("paths-self-choice.ts");
        std::fs::write(
            &paths_self_choice,
            "export interface SelfChoiceProps { selfPathsValue: number }",
        )
        .expect("write self-name paths target");
        std::fs::write(
            source_dir.join("imports-choice.ts"),
            "export interface ChoiceProps { wrongImportsPriority: never }",
        )
        .expect("write imports decoy");
        std::fs::write(
            source_dir.join("self-choice.ts"),
            "export interface SelfChoiceProps { wrongSelfPriority: never }",
        )
        .expect("write self-name decoy");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ChoiceProps } from '#choice'
import type { SelfChoiceProps } from 'vuec-path-priority/choice'
defineProps<ChoiceProps & SelfChoiceProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("pathsValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("selfPathsValue: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongImportsPriority"));
        assert!(!script.content.contains("wrongSelfPriority"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [paths_choice, paths_self_choice]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_project_package_imports_map_emitted_targets_back_to_sources() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source_dir = dir.path().join("src");
        std::fs::create_dir_all(&source_dir).expect("create project source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "imports": {
                    "#javascript": "./dist/javascript.js",
                    "#declaration": "./declarations/declaration.d.ts",
                    "#module": "./dist/module.mjs",
                    "#commonjs": "./dist/commonjs.cjs"
                }
            }"##,
        )
        .expect("write project imports manifest");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "rootDir": "./src",
                    "outDir": "./dist",
                    "declarationDir": "./declarations"
                }
            }"#,
        )
        .expect("write project config");
        std::fs::write(
            source_dir.join("javascript.ts"),
            "export interface JavaScriptProps { wrongTsPriority: never }",
        )
        .expect("write lower-priority project source target");
        let targets = [
            (
                source_dir.join("javascript.tsx"),
                "export interface JavaScriptProps { javascriptValue: string }",
            ),
            (
                source_dir.join("declaration.ts"),
                "export interface DeclarationProps { declarationValue: number }",
            ),
            (
                source_dir.join("module.mts"),
                "export interface ModuleProps { moduleValue: boolean }",
            ),
            (
                source_dir.join("commonjs.cts"),
                "export interface CommonJsProps { commonJsValue: string }",
            ),
        ];
        for (path, source) in &targets {
            std::fs::write(path, source).expect("write project source target");
        }

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { JavaScriptProps } from '#javascript'
import type { DeclarationProps } from '#declaration'
import type { ModuleProps } from '#module'
import type { CommonJsProps } from '#commonjs'
defineProps<JavaScriptProps & DeclarationProps & ModuleProps & CommonJsProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "javascriptValue: { type: String, required: true }",
            "declarationValue: { type: Number, required: true }",
            "moduleValue: { type: Boolean, required: true }",
            "commonJsValue: { type: String, required: true }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = targets
            .iter()
            .map(|(path, _)| normalize_path_string(path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.content.contains("wrongTsPriority"), "{}", script.content);
    }

    #[test]
    fn vue3_project_package_imports_inherit_emit_paths_from_their_declaring_configs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config_dir = dir.path().join("configs");
        let source_dir = dir.path().join("sources");
        std::fs::create_dir_all(&config_dir).expect("create config directory");
        std::fs::create_dir_all(&source_dir).expect("create source directory");
        std::fs::write(
            dir.path().join("package.json"),
            r##"{
                "imports": {
                    "#output": "./dist/output.js",
                    "#declaration": "./declarations/declaration.d.ts"
                }
            }"##,
        )
        .expect("write project imports manifest");
        std::fs::write(
            config_dir.join("base.json"),
            r#"{
                "compilerOptions": {
                    "rootDir": "../sources",
                    "outDir": "../base-dist"
                }
            }"#,
        )
        .expect("write base config");
        std::fs::write(
            config_dir.join("declarations.json"),
            r#"{
                "compilerOptions": {
                    "declarationDir": "../declarations"
                }
            }"#,
        )
        .expect("write declaration config");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": [
                    "./configs/base.json",
                    "./configs/declarations.json"
                ],
                "compilerOptions": {
                    "outDir": "./dist"
                }
            }"#,
        )
        .expect("write project config");
        let output = source_dir.join("output.ts");
        let declaration = source_dir.join("declaration.ts");
        std::fs::write(
            &output,
            "export interface OutputProps { outputValue: string }",
        )
        .expect("write inherited output source");
        std::fs::write(
            &declaration,
            "export interface DeclarationProps { declarationValue: number }",
        )
        .expect("write inherited declaration source");

        let filename = source_dir.join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { OutputProps } from '#output'
import type { DeclarationProps } from '#declaration'
defineProps<OutputProps & DeclarationProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("outputValue: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(
            script
                .content
                .contains("declarationValue: { type: Number, required: true }"),
            "{}",
            script.content
        );
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            [output, declaration]
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_project_package_input_candidates_match_typescript_emit_extensions() {
        let path = Path::new("project/dist/entry.d.ts");
        assert_eq!(
            vue3_possible_project_input_paths(path),
            [
                "project/dist/entry.tsx",
                "project/dist/entry.ts",
                "project/dist/entry.jsx",
                "project/dist/entry.js",
            ]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
        );
        assert_eq!(
            vue3_possible_project_input_paths(Path::new("project/dist/entry.d.mts")),
            ["project/dist/entry.mts", "project/dist/entry.mjs"]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            vue3_possible_project_input_paths(Path::new("project/dist/entry.d.cts")),
            ["project/dist/entry.cts", "project/dist/entry.cjs"]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
        assert!(vue3_possible_project_input_paths(Path::new("project/dist/entry.ts")).is_empty());
    }

    #[test]
    fn vue3_package_relative_targets_normalize_windows_separators() {
        let resolver = Vue3TypeResolverContext::default();
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({ "types": "./types\\index.d.ts" }),
                None,
                &resolver,
            )
            .as_deref(),
            Some("./types/index.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({
                    "./feature/*": { "types": "./types\\*.d.ts" }
                }),
                Some("feature/item"),
                &resolver,
            )
            .as_deref(),
            Some("./types/item.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(
                &serde_json::json!({ "./legacy/": { "types": "./types\\" } }),
                Some("legacy/item.d.ts"),
                &resolver,
            )
            .as_deref(),
            Some("./types/item.d.ts")
        );
        assert!(vue3_package_exports_type_target(
            &serde_json::json!({ "types": ".\\types\\index.d.ts" }),
            None,
            &resolver,
        )
        .is_none());
        assert!(!vue3_package_import_external_target_is_safe(
            "vuec-external\\feature"
        ));

        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("node_modules").join("vuec-backslash-targets");
        let types = package.join("types");
        std::fs::create_dir_all(&types).expect("create package types directory");
        std::fs::write(
            package.join("package.json"),
            r##"{
                "name":"vuec-backslash-targets",
                "exports":{
                    ".":{"types":"./types\\index.d.ts"},
                    "./feature/*":{"types":"./types\\*.d.ts"}
                },
                "imports":{"#local":{"types":"./types\\index.d.ts"}}
            }"##,
        )
        .expect("write backslash target manifest");
        let index = types.join("index.d.ts");
        let item = types.join("item.d.ts");
        let importer = package.join("source.d.mts");
        std::fs::write(&index, "export interface Index {}").expect("write root type target");
        std::fs::write(&item, "export interface Item {}").expect("write pattern type target");
        std::fs::write(&importer, "export {};").expect("write package importer");

        assert_eq!(
            resolve_vue3_package_json_type_entry(&package, None, &resolver),
            Vue3PackageJsonTypeResolution::Resolved(index.clone())
        );
        assert_eq!(
            resolve_vue3_package_json_type_entry(&package, Some("feature/item"), &resolver),
            Vue3PackageJsonTypeResolution::Resolved(item)
        );
        assert_eq!(
            resolve_vue3_type_import(&importer.to_string_lossy(), "#local", &resolver),
            Some(index)
        );
    }

    #[test]
    fn vue3_package_exports_selects_the_most_specific_pattern() {
        let resolver = Vue3TypeResolverContext::default();
        let invalid_array_fallback = serde_json::json!([
            "../invalid.d.ts",
            "./valid.d.ts"
        ]);
        assert_eq!(
            vue3_package_exports_type_target(&invalid_array_fallback, None, &resolver).as_deref(),
            Some("./valid.d.ts")
        );
        let null_array_fallback = serde_json::json!([null, "./valid.d.ts"]);
        assert_eq!(
            vue3_package_exports_type_target(&null_array_fallback, None, &resolver).as_deref(),
            Some("./valid.d.ts")
        );
        assert!(vue3_package_exports_type_target(
            &serde_json::json!([null]),
            None,
            &resolver,
        )
        .is_none());
        assert!(vue3_package_exports_type_target(
            &serde_json::json!([]),
            None,
            &resolver,
        )
        .is_none());
        let legacy_prefix = serde_json::json!({
            "./legacy/": { "types": "./types/" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &legacy_prefix,
                Some("legacy/item.d.ts"),
                &resolver,
            )
            .as_deref(),
            Some("./types/item.d.ts")
        );
        let invalid_legacy_prefix = serde_json::json!({
            "./legacy/": { "types": "./types/index.d.ts" }
        });
        assert!(vue3_package_exports_type_target(
            &invalid_legacy_prefix,
            Some("legacy/item.d.ts"),
            &resolver,
        )
        .is_none());
        let pattern_over_prefix = serde_json::json!({
            "./legacy/": { "types": "./prefix/" },
            "./legacy/*": { "types": "./pattern/*.d.ts" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &pattern_over_prefix,
                Some("legacy/item"),
                &resolver,
            )
            .as_deref(),
            Some("./pattern/item.d.ts")
        );

        let exports = serde_json::json!({
            "./feature/*": { "types": "./generic/*.d.ts" },
            "./feature/internal/*": { "types": "./internal/*.d.ts" },
            "./feature/*.js": { "types": "./javascript/*.d.ts" },
            "./feature/exact.js": { "types": "./exact.d.ts" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &exports,
                Some("feature/internal/item"),
                &resolver,
            )
            .as_deref(),
            Some("./internal/item.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(&exports, Some("feature/item.js"), &resolver)
                .as_deref(),
            Some("./javascript/item.d.ts")
        );
        assert_eq!(
            vue3_package_exports_type_target(&exports, Some("feature/exact.js"), &resolver)
                .as_deref(),
            Some("./exact.d.ts")
        );

        let exclusions = serde_json::json!({
            "./*": { "types": "./broad/*.d.ts" },
            "./private/*": null,
            "./private/exact": null
        });
        assert!(vue3_package_exports_type_target(
            &exclusions,
            Some("private/item"),
            &resolver,
        )
        .is_none());
        assert!(vue3_package_exports_type_target(
            &exclusions,
            Some("private/exact"),
            &resolver,
        )
        .is_none());

        let invalid_pattern = serde_json::json!({
            "./feature/*": { "types": "./broad/*.d.ts" },
            "./feature/*/*": { "types": "./invalid/*.d.ts" }
        });
        assert_eq!(
            vue3_package_exports_type_target(
                &invalid_pattern,
                Some("feature/one/two"),
                &resolver,
            )
            .as_deref(),
            Some("./broad/one/two.d.ts")
        );
        assert!(vue3_package_export_pattern_capture("./feature/*/*", "./feature/one/two")
            .is_none());
    }

    #[test]
    fn vue3_package_exports_reject_invalid_object_shapes() {
        let resolver = Vue3TypeResolverContext::default();
        for mixed_keys in [
            serde_json::json!({
                ".": "./root.d.ts",
                "./feature": "./feature.d.ts",
                "types": "./conditional.d.ts"
            }),
            serde_json::json!({
                "types": "./conditional.d.ts",
                ".": "./root.d.ts",
                "./feature": "./feature.d.ts"
            }),
        ] {
            assert!(vue3_package_exports_type_target(&mixed_keys, None, &resolver).is_none());
            assert!(vue3_package_exports_type_target(
                &mixed_keys,
                Some("feature"),
                &resolver,
            )
            .is_none());
        }

        let numeric_condition = serde_json::json!({
            "types": "./valid.d.ts",
            "0": "./invalid.d.ts"
        });
        assert!(
            vue3_package_exports_type_target(&numeric_condition, None, &resolver).is_none()
        );
        let nested_numeric_condition = serde_json::json!({
            "types": { "0": "./invalid.d.ts" },
            "default": "./fallback.d.ts"
        });
        assert!(vue3_package_exports_type_target(
            &nested_numeric_condition,
            None,
            &resolver,
        )
        .is_none());
        let numeric_condition_in_array = serde_json::json!([
            { "4294967294": "./invalid.d.ts" },
            "./fallback.d.ts"
        ]);
        assert!(vue3_package_exports_type_target(
            &numeric_condition_in_array,
            None,
            &resolver,
        )
        .is_none());

        for condition in ["00", "-0", "4294967295", "1e0"] {
            let conditions = serde_json::json!({
                (condition): "./inactive.d.ts",
                "default": "./valid.d.ts"
            });
            assert_eq!(
                vue3_package_exports_type_target(&conditions, None, &resolver).as_deref(),
                Some("./valid.d.ts"),
                "{condition}"
            );
        }

        for conditions in [
            serde_json::json!({
                ".": {
                    "./unknown": "../invalid.d.ts",
                    "types": "./valid.d.ts"
                }
            }),
            serde_json::json!({
                ".": {
                    "types": "./valid.d.ts",
                    "./unknown": "../invalid.d.ts"
                }
            }),
        ] {
            assert_eq!(
                vue3_package_exports_type_target(&conditions, None, &resolver).as_deref(),
                Some("./valid.d.ts")
            );
        }
    }

    #[test]
    fn vue3_package_exports_select_only_the_requested_resolution_mode() {
        let resolver = Vue3TypeResolverContext::default();
        let exports = serde_json::json!({
            ".": {
                "types": {
                    "import": "./import.d.mts",
                    "require": "./require.d.cts"
                }
            },
            "./feature/*": {
                "types": {
                    "import": "./import/*.d.mts",
                    "require": "./require/*.d.cts"
                }
            }
        });

        for (mode, root, pattern) in [
            (
                Vue3TypeResolutionMode::Import,
                "./import.d.mts",
                "./import/item.d.mts",
            ),
            (
                Vue3TypeResolutionMode::Require,
                "./require.d.cts",
                "./require/item.d.cts",
            ),
        ] {
            assert_eq!(
                vue3_package_exports_type_target_with_mode(&exports, None, mode, &resolver)
                    .as_deref(),
                Some(root),
            );
            assert_eq!(
                vue3_package_exports_type_target_with_mode(
                    &exports,
                    Some("feature/item"),
                    mode,
                    &resolver,
                )
                .as_deref(),
                Some(pattern),
            );
        }

        let require_only = serde_json::json!({ ".": { "require": "./require.d.cts" } });
        assert!(vue3_package_exports_type_target(
            &require_only,
            None,
            &resolver,
        )
        .is_none());
    }

    #[test]
    fn vue3_resolution_mode_attributes_drive_imported_and_re_exported_macro_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-resolution-mode-attributes");
        std::fs::create_dir_all(&package).expect("create conditional package");
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
        .expect("write conditional package manifest");
        let import_entry = package.join("import.d.mts");
        let require_entry = package.join("require.d.cts");
        std::fs::write(
            &import_entry,
            "export interface CommonJsImportedSource { commonJsImported: string }",
        )
        .expect("write import condition types");
        std::fs::write(
            &require_entry,
            r#"
export interface DirectRequired { directRequired: string }
export interface NamedRequiredSource { namedRequired: number }
export interface AllRequired { allRequired: boolean }
export interface CommonJsDefaultImportedSource { commonJsDefaultImported: number }
export interface CommonJsDefaultImportTypeSource { commonJsDefaultImportType: boolean }
export interface ImportTypeRequired { importTypeRequired: string }
"#,
        )
        .expect("write require condition types");

        let named_bridge = dir.path().join("named-bridge.d.ts");
        let all_bridge = dir.path().join("all-bridge.d.ts");
        let commonjs_bridge = dir.path().join("commonjs-bridge.d.cts");
        let commonjs_default_bridge = dir.path().join("commonjs-default-bridge.d.cts");
        std::fs::write(
            &named_bridge,
            r#"export type { NamedRequiredSource as NamedRequired } from 'vuec-resolution-mode-attributes' with { "resolution-mode": "require" }"#,
        )
        .expect("write named require bridge");
        std::fs::write(
            &all_bridge,
            r#"export type * from 'vuec-resolution-mode-attributes' with { "resolution-mode": "require" }"#,
        )
        .expect("write export-all require bridge");
        std::fs::write(
            &commonjs_bridge,
            r#"export type { CommonJsImportedSource as CommonJsImported } from 'vuec-resolution-mode-attributes' with { "resolution-mode": "import" }"#,
        )
        .expect("write CommonJS import bridge");
        std::fs::write(
            &commonjs_default_bridge,
            r#"
import type { CommonJsDefaultImportedSource } from 'vuec-resolution-mode-attributes'
export type CommonJsDefaultImported = CommonJsDefaultImportedSource
export type CommonJsDefaultImportType = import('vuec-resolution-mode-attributes').CommonJsDefaultImportTypeSource
"#,
        )
        .expect("write CommonJS default-mode bridge");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { DirectRequired } from 'vuec-resolution-mode-attributes' with { "resolution-mode": "require" }
import type { NamedRequired } from './named-bridge'
import type { AllRequired } from './all-bridge'
import type { CommonJsImported } from './commonjs-bridge.d.cts'
import type { CommonJsDefaultImported, CommonJsDefaultImportType } from './commonjs-default-bridge.d.cts'
type ImportTypeRequired = import('vuec-resolution-mode-attributes', { with: { "resolution-mode": `require` } }).ImportTypeRequired
defineProps<DirectRequired & NamedRequired & AllRequired & CommonJsImported & CommonJsDefaultImported & CommonJsDefaultImportType & ImportTypeRequired>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("directRequired: { type: String, required: true }"));
        assert!(script
            .content
            .contains("namedRequired: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("allRequired: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("commonJsImported: { type: String, required: true }"));
        assert!(script
            .content
            .contains("commonJsDefaultImported: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("commonJsDefaultImportType: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("importTypeRequired: { type: String, required: true }"));
        for dependency in [
            import_entry,
            require_entry,
            named_bridge,
            all_bridge,
            commonjs_bridge,
            commonjs_default_bridge,
        ] {
            assert!(
                script.deps.contains(&normalize_path_string(&dependency)),
                "missing dependency {}",
                dependency.display()
            );
        }
    }

    #[test]
    fn vue3_package_type_drives_transitive_resolution_modes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let conditional = node_modules.join("vuec-package-type-conditional");
        std::fs::create_dir_all(&conditional).expect("create conditional package");
        std::fs::write(
            conditional.join("package.json"),
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
        .expect("write conditional package manifest");
        let import_entry = conditional.join("import.d.mts");
        let require_entry = conditional.join("require.d.cts");
        std::fs::write(
            &import_entry,
            r#"
export interface ImportDirect { importDirect: string }
export interface ImportType { importType: number }
export interface ImportNamed { importNamed: boolean }
export interface ImportAll { importAll: bigint }
export interface ImportExplicit { importExplicit: symbol }
export interface ImportBoundary { importBoundary: object }
export interface ImportGeneric { importGeneric: string }
export interface WrongRequireDirect { wrongRequireDirect: string }
"#,
        )
        .expect("write import condition types");
        std::fs::write(
            &require_entry,
            r#"
export interface RequireDirect { requireDirect: string }
export interface RequireType { requireType: number }
export interface RequireNamed { requireNamed: boolean }
export interface RequireAll { requireAll: bigint }
export interface RequireExplicit { requireExplicit: symbol }
export interface RequireBoundary { requireBoundary: object }
export interface RequireGeneric { requireGeneric: string }
export interface WrongImportDirect { wrongImportDirect: string }
"#,
        )
        .expect("write require condition types");

        let commonjs_bridge = node_modules.join("vuec-commonjs-type-bridge");
        std::fs::create_dir_all(&commonjs_bridge).expect("create CommonJS bridge");
        std::fs::write(
            commonjs_bridge.join("package.json"),
            r#"{"type":"commonjs","types":"index.d.ts"}"#,
        )
        .expect("write CommonJS bridge manifest");
        std::fs::write(
            commonjs_bridge.join("index.d.ts"),
            r#"
import type { RequireDirect } from 'vuec-package-type-conditional'
export interface CommonJsDirect extends RequireDirect {}
export type CommonJsImportType = import('vuec-package-type-conditional').RequireType
export type CommonJsGeneric<T> = T & import('vuec-package-type-conditional').RequireGeneric
export type { RequireNamed as CommonJsNamed } from 'vuec-package-type-conditional'
export * from 'vuec-package-type-conditional'
"#,
        )
        .expect("write CommonJS bridge types");

        let module_bridge = node_modules.join("vuec-module-type-bridge");
        std::fs::create_dir_all(&module_bridge).expect("create module bridge");
        std::fs::write(
            module_bridge.join("package.json"),
            r#"{"type":"module","types":"index.d.ts"}"#,
        )
        .expect("write module bridge manifest");
        std::fs::write(
            module_bridge.join("index.d.ts"),
            r#"
import type { ImportDirect } from 'vuec-package-type-conditional'
export interface ModuleDirect extends ImportDirect {}
export type ModuleImportType = import('vuec-package-type-conditional').ImportType
export type { ImportNamed as ModuleNamed } from 'vuec-package-type-conditional'
export * from 'vuec-package-type-conditional'
"#,
        )
        .expect("write module bridge types");

        let explicit_commonjs = node_modules.join("vuec-explicit-commonjs-bridge");
        std::fs::create_dir_all(&explicit_commonjs).expect("create explicit CommonJS bridge");
        std::fs::write(
            explicit_commonjs.join("package.json"),
            r#"{"type":"module","types":"index.d.cts"}"#,
        )
        .expect("write explicit CommonJS bridge manifest");
        std::fs::write(
            explicit_commonjs.join("index.d.cts"),
            "export type ExplicitCommonJs = import('vuec-package-type-conditional').RequireExplicit",
        )
        .expect("write explicit CommonJS bridge types");

        let explicit_module = node_modules.join("vuec-explicit-module-bridge");
        std::fs::create_dir_all(&explicit_module).expect("create explicit module bridge");
        std::fs::write(
            explicit_module.join("package.json"),
            r#"{"type":"commonjs","types":"index.d.mts"}"#,
        )
        .expect("write explicit module bridge manifest");
        std::fs::write(
            explicit_module.join("index.d.mts"),
            "export type ExplicitModule = import('vuec-package-type-conditional').ImportExplicit",
        )
        .expect("write explicit module bridge types");

        let nested_boundary = node_modules.join("vuec-nested-package-boundary");
        let nested = nested_boundary.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested package boundary");
        std::fs::write(
            nested_boundary.join("package.json"),
            r#"{"type":"module","types":"nested/index.d.ts"}"#,
        )
        .expect("write outer module package manifest");
        std::fs::write(nested.join("package.json"), "{}")
            .expect("write empty nested package manifest");
        std::fs::write(
            nested.join("index.d.ts"),
            "export type BoundaryType = import('vuec-package-type-conditional').RequireBoundary",
        )
        .expect("write nested boundary types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { CommonJsDirect, CommonJsGeneric, CommonJsImportType, CommonJsNamed, RequireAll } from 'vuec-commonjs-type-bridge'
import type { ModuleDirect, ModuleImportType, ModuleNamed, ImportAll } from 'vuec-module-type-bridge'
import type { ExplicitCommonJs } from 'vuec-explicit-commonjs-bridge'
import type { ExplicitModule } from 'vuec-explicit-module-bridge'
import type { BoundaryType } from 'vuec-nested-package-boundary'
defineProps<CommonJsDirect & CommonJsGeneric<{ genericLocal: number }> & CommonJsImportType & CommonJsNamed & RequireAll & ModuleDirect & ModuleImportType & ModuleNamed & ImportAll & ExplicitCommonJs & ExplicitModule & BoundaryType>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for property in [
            "requireDirect",
            "requireType",
            "requireNamed",
            "requireAll",
            "importDirect",
            "importType",
            "importNamed",
            "importAll",
            "requireExplicit",
            "requireGeneric",
            "genericLocal",
            "importExplicit",
            "requireBoundary",
        ] {
            assert!(
                script.content.contains(&format!("{property}: {{ type:")),
                "missing {property}: {}",
                script.content
            );
        }
        assert!(!script.content.contains("wrongRequireDirect:"));
        assert!(!script.content.contains("wrongImportDirect:"));
        assert!(!script.content.contains("importGeneric:"));
        assert!(!script.content.contains("importBoundary:"));
        for dependency in [
            import_entry,
            require_entry,
            commonjs_bridge.join("index.d.ts"),
            module_bridge.join("index.d.ts"),
            explicit_commonjs.join("index.d.cts"),
            explicit_module.join("index.d.mts"),
            nested.join("index.d.ts"),
        ] {
            assert!(
                script.deps.contains(&normalize_path_string(&dependency)),
                "missing dependency {}",
                dependency.display()
            );
        }
    }

    #[test]
    fn vue3_package_types_version_selector_supports_node_semver_ranges() {
        for selector in [
            "*",
            "<=5.0",
            "~5.0",
            "^4.8 || >=5.0",
            "5.0 - 5.9",
            ">=4.8 <5.3",
            "5.x",
            "5.*",
        ] {
            assert!(
                vue3_package_types_version_selector_matches(selector),
                "{selector}"
            );
        }

        for selector in ["", ">=5.1", "<5.0", "4.x", "4.*", "5.1 - 5.9"] {
            assert!(
                !vue3_package_types_version_selector_matches(selector),
                "{selector}"
            );
        }
    }

    #[test]
    fn vue3_package_types_versions_targets_accept_windows_separators_cross_platform() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir
            .path()
            .join("node_modules")
            .join("vuec-typesversions-windows-paths");
        let target = package.join("versioned").join("feature").join("item.d.ts");
        std::fs::create_dir_all(target.parent().expect("target parent"))
            .expect("create versioned package directory");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "*": {
                        "feature/*": ["versioned\\feature\\*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            package.join("index.d.ts"),
            "export interface VersionedProps { wrongFallback: never }",
        )
        .expect("write versioned fallback types");
        std::fs::write(
            &target,
            "export interface VersionedProps { windowsTarget: string }",
        )
        .expect("write versioned Windows target");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { VersionedProps } from 'vuec-typesversions-windows-paths/feature/item'
defineProps<VersionedProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("windowsTarget: { type: String, required: true }"),
            "{}",
            script.content
        );
        assert!(!script.content.contains("wrongFallback"));
        assert_eq!(script.deps, vec![normalize_path_string(&target)]);
    }

    #[test]
    fn vue3_package_root_fields_normalize_windows_separators_before_version_matching() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let mut expected_deps = Vec::new();
        for (package_name, manifest, relative_target, declaration) in [
            (
                "vuec-windows-types-field",
                r#"{"types":".\\declarations\\index.d.ts"}"#,
                "declarations/index.d.ts",
                "export interface TypesFieldProps { typesField: string }",
            ),
            (
                "vuec-windows-typings-field",
                r#"{"typings":".\\declarations\\index.d.ts"}"#,
                "declarations/index.d.ts",
                "export interface TypingsFieldProps { typingsField: number }",
            ),
            (
                "vuec-windows-main-field",
                r#"{"main":".\\dist\\index.js"}"#,
                "dist/index.d.ts",
                "export interface MainFieldProps { mainField: boolean }",
            ),
        ] {
            let package = node_modules.join(package_name);
            let target = package.join(relative_target);
            std::fs::create_dir_all(target.parent().expect("package target parent"))
                .expect("create package root field fixture");
            std::fs::write(package.join("package.json"), manifest)
                .expect("write package root field manifest");
            std::fs::write(&target, declaration).expect("write package root field declaration");
            expected_deps.push(target);
        }

        let versioned_package = node_modules.join("vuec-windows-versioned-root");
        let fallback = versioned_package.join("types").join("index.d.ts");
        let versioned = versioned_package.join("versioned").join("index.d.ts");
        for target in [&fallback, &versioned] {
            std::fs::create_dir_all(target.parent().expect("versioned target parent"))
                .expect("create versioned root fixture");
        }
        std::fs::write(
            versioned_package.join("package.json"),
            r#"{
                "types": ".\\types\\index.d.ts",
                "typesVersions": {
                    "*": { "types/*": ["versioned\\*"] }
                }
            }"#,
        )
        .expect("write versioned root manifest");
        std::fs::write(
            &fallback,
            "export interface VersionedRootProps { wrongFallback: never }",
        )
        .expect("write versioned root fallback");
        std::fs::write(
            &versioned,
            "export interface VersionedRootProps { versionedRoot?: string }",
        )
        .expect("write versioned root target");
        expected_deps.push(versioned);

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { TypesFieldProps } from 'vuec-windows-types-field'
import type { TypingsFieldProps } from 'vuec-windows-typings-field'
import type { MainFieldProps } from 'vuec-windows-main-field'
import type { VersionedRootProps } from 'vuec-windows-versioned-root'
defineProps<TypesFieldProps & TypingsFieldProps & MainFieldProps & VersionedRootProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected in [
            "typesField: { type: String, required: true }",
            "typingsField: { type: Number, required: true }",
            "mainField: { type: Boolean, required: true }",
            "versionedRoot: { type: String, required: false }",
        ] {
            assert!(script.content.contains(expected), "{}", script.content);
        }
        assert!(!script.content.contains("wrongFallback"));
        assert_eq!(
            script.deps.iter().cloned().collect::<BTreeSet<_>>(),
            expected_deps
                .iter()
                .map(|path| normalize_path_string(path))
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn vue3_compile_script_resolves_package_types_versions_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let versioned_pkg = node_modules.join("vuec-typesversions-pkg");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("future").join("feature"))
            .expect("create future types");
        std::fs::create_dir_all(versioned_pkg.join("ts5").join("feature"))
            .expect("create ts5 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["future/index.d.ts"],
                        "feature/*": ["future/feature/*.d.ts"]
                    },
                    "^4.8 || 5.x": {
                        "dist/index.d.ts": ["ts5/index.d.ts"],
                        "feature/*": ["ts5/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface RootProps { fallbackRoot: string }",
        )
        .expect("write fallback root types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface RootProps { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("future").join("index.d.ts"),
            "export interface RootProps { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write future root types");
        std::fs::write(
            versioned_pkg
                .join("future")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { futureFeature: string }",
        )
        .expect("write future feature types");
        std::fs::write(
            versioned_pkg.join("future").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write future model types");
        std::fs::write(
            versioned_pkg.join("ts5").join("index.d.ts"),
            "export interface RootProps { root: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts5 root types");
        std::fs::write(
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature?: number }",
        )
        .expect("write ts5 feature types");
        std::fs::write(
            versioned_pkg.join("ts5").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts5 model types");

        let ambient_pkg = node_modules
            .join("@types")
            .join("vuec-typesversions-ambient");
        std::fs::create_dir_all(ambient_pkg.join("ts5")).expect("create @types versioned");
        std::fs::write(
            ambient_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "~5.0": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write @types package manifest");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambientFallback: number }",
        )
        .expect("write fallback @types");
        std::fs::write(
            ambient_pkg.join("ts5").join("index.d.ts"),
            "export type AmbientProps = { ambient: boolean }",
        )
        .expect("write ts5 @types");

        let type_root_pkg = dir.path().join("typings").join("versioned-global");
        std::fs::create_dir_all(type_root_pkg.join("ts5")).expect("create type root package");
        std::fs::write(
            type_root_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "5.0 - 5.9": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write type root package manifest");
        std::fs::write(
            type_root_pkg.join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRootFallback: number }",
        )
        .expect("write fallback type root global");
        std::fs::write(
            type_root_pkg.join("ts5").join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRoot: string }",
        )
        .expect("write ts5 type root global");

        let ordered_pkg = node_modules.join("vuec-typesversions-ordered");
        std::fs::create_dir_all(ordered_pkg.join("first")).expect("create first ordered types");
        std::fs::create_dir_all(ordered_pkg.join("second")).expect("create second ordered types");
        std::fs::create_dir_all(ordered_pkg.join("fallback"))
            .expect("create fallback ordered types");
        std::fs::write(
            ordered_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    ">=4.8": {
                        "index.d.ts": ["first/index.d.ts"]
                    },
                    ">=5.0": {
                        "index.d.ts": ["second/index.d.ts"]
                    },
                    "*": {
                        "index.d.ts": ["fallback/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write ordered package manifest");
        std::fs::write(
            ordered_pkg.join("index.d.ts"),
            "export type OrderedProps = { orderedFallbackRoot: boolean }",
        )
        .expect("write ordered root fallback");
        std::fs::write(
            ordered_pkg.join("first").join("index.d.ts"),
            "export type OrderedProps = { orderedFirst: string }",
        )
        .expect("write first ordered types");
        std::fs::write(
            ordered_pkg.join("second").join("index.d.ts"),
            "export type OrderedProps = { orderedSecond: number }",
        )
        .expect("write second ordered types");
        std::fs::write(
            ordered_pkg.join("fallback").join("index.d.ts"),
            "export type OrderedProps = { orderedFallback: boolean }",
        )
        .expect("write fallback ordered types");

        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "types": ["versioned-global"],
                    "typeRoots": ["./typings"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'vuec-typesversions-pkg'
import type { FeatureProps } from 'vuec-typesversions-pkg/feature/item'
import type { AmbientProps } from 'vuec-typesversions-ambient'
import type { OrderedProps } from 'vuec-typesversions-ordered'
defineProps<RootProps & FeatureProps & AmbientProps & TypeRootGlobalProps & OrderedProps>()
defineModel<import('vuec-typesversions-pkg').ModelValue>()
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
            .contains("feature: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("ambient: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("typeRoot: { type: String, required: true }"));
        assert!(script
            .content
            .contains("orderedFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!script.content.contains("fallbackRoot"));
        assert!(!script.content.contains("futureRoot"));
        assert!(!script.content.contains("futureFeature"));
        assert!(!script.content.contains("legacyRoot"));
        assert!(!script.content.contains("legacyFeature"));
        assert!(!script.content.contains("ambientFallback"));
        assert!(!script.content.contains("typeRootFallback"));
        assert!(!script.content.contains("orderedSecond"));
        assert!(!script.content.contains("orderedFallback"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts5").join("index.d.ts"),
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts5").join("model.d.ts"),
            ambient_pkg.join("ts5").join("index.d.ts"),
            type_root_pkg.join("ts5").join("index.d.ts"),
            ordered_pkg.join("first").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_package_types_versions_from_project_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let typescript_pkg = node_modules.join("typescript");
        std::fs::create_dir_all(&typescript_pkg).expect("create typescript package");
        std::fs::write(
            typescript_pkg.join("package.json"),
            r#"{"version":"5.2.0"}"#,
        )
        .expect("write typescript manifest");

        let versioned_pkg = node_modules.join("vuec-typesversions-project-ts");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("ts52").join("feature"))
            .expect("create ts52 types");
        std::fs::create_dir_all(versioned_pkg.join("ts50").join("feature"))
            .expect("create ts50 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["ts52/index.d.ts"],
                        "feature/*": ["ts52/feature/*.d.ts"]
                    },
                    ">=5.0": {
                        "dist/index.d.ts": ["ts50/index.d.ts"],
                        "feature/*": ["ts50/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface Props { fallbackRoot: string }",
        )
        .expect("write dist fallback types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface Props { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("index.d.ts"),
            "export interface Props { baselineRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts50 root types");
        std::fs::write(
            versioned_pkg.join("ts50").join("feature").join("item.d.ts"),
            "export type FeatureProps = { baselineFeature: boolean }",
        )
        .expect("write ts50 feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts50 model types");
        std::fs::write(
            versioned_pkg.join("ts52").join("index.d.ts"),
            "export interface Props { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts52 root types");
        std::fs::write(
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            "export type FeatureProps = { futureFeature?: number }",
        )
        .expect("write ts52 feature types");
        std::fs::write(
            versioned_pkg.join("ts52").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write ts52 model types");

        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props } from 'vuec-typesversions-project-ts'
import type { FeatureProps } from 'vuec-typesversions-project-ts/feature/item'
defineProps<Props & FeatureProps>()
defineModel<import('vuec-typesversions-project-ts').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("futureRoot: { type: String, required: true }"));
        assert!(script
            .content
            .contains("futureFeature: { type: Number, required: false }"));
        assert!(script.content.contains("\"modelValue\": { type: Number },"));
        assert!(!script.content.contains("baselineRoot"));
        assert!(!script.content.contains("baselineFeature"));
        assert!(!script.content.contains("legacyRoot"));
        assert!(!script.content.contains("legacyFeature"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts52").join("index.d.ts"),
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts52").join("model.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }
