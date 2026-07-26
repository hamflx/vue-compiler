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
    fn vue3_package_exports_selects_the_most_specific_pattern() {
        let resolver = Vue3TypeResolverContext::default();
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
