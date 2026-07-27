    #[test]
    fn vue3_sfc_bridge_compile_script_returns_re_exported_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-re-export-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("leaf.ts"), "export type Props = { foo: string }")
            .expect("write leaf");
        std::fs::write(
            dir.join("bar.ts"),
            "export { Props as PublicProps } from './leaf'",
        )
        .expect("write bar");
        std::fs::write(
            dir.join("foo.ts"),
            "export { PublicProps as Props } from './bar'",
        )
        .expect("write foo");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './foo'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["foo.ts", "bar.ts", "leaf.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_default_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-default-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("leaf.ts"),
            "export default interface Props { foo: string }",
        )
        .expect("write leaf");
        std::fs::write(dir.join("bar.ts"), "export { default } from './leaf'").expect("write bar");
        std::fs::write(
            dir.join("named.ts"),
            "export interface NamedProps { bar?: number }",
        )
        .expect("write named");
        std::fs::write(
            dir.join("baz.ts"),
            "export { NamedProps as default } from './named'",
        )
        .expect("write baz");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import Props from './bar'\n",
                    "import ExtraProps from './baz'\n",
                    "defineProps<Props & ExtraProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["bar.ts", "leaf.ts", "baz.ts", "named.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("bar: { type: Number, required: false }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_class_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-class-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("classes.ts"),
            "export class NamedClass {}\nexport type Props = { named: NamedClass }",
        )
        .expect("write class types");
        std::fs::write(dir.join("leaf.ts"), "export default class DefaultClass {}")
            .expect("write default class leaf");
        std::fs::write(dir.join("bar.ts"), "export { default } from './leaf'")
            .expect("write default class facade");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import DefaultClass from './bar'\n",
                    "import type { Props } from './classes'\n",
                    "class LocalClass {}\n",
                    "defineProps<{ local: LocalClass, external: Props, value: DefaultClass }>()\n",
                    "defineModel<DefaultClass>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["classes.ts", "bar.ts", "leaf.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("local: { type: Object, required: true }"));
        assert!(content.contains("external: { type: Object, required: true }"));
        assert!(content.contains("value: { type: Object, required: true }"));
        assert!(content.contains("\"modelValue\": { type: Object },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_enum_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-enum-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("enums.ts"),
            "export enum Kind { A = 'a', B = 'b' }\nexport enum Code { A = 1, B = 2 }\nexport enum Mixed { A = 'a', B = 1 }\nexport enum Auto { A, B }\nexport type Props = { kind: Kind, code?: Code, mixed: Mixed, auto: Auto }\nexport type ModelValue = Kind | Code",
        )
        .expect("write enums");
        std::fs::write(
            dir.join("facade.ts"),
            "export { Props as FacadeProps, ModelValue as FacadeModel } from './enums'",
        )
        .expect("write facade");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { FacadeProps, FacadeModel } from './facade'\n",
                    "const props = defineProps<FacadeProps>()\n",
                    "const model = defineModel<FacadeModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["enums.ts", "facade.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("kind: { type: String, required: true }"));
        assert!(content.contains("code: { type: Number, required: false }"));
        assert!(content.contains("mixed: { type: [String, Number], required: true }"));
        assert!(content.contains("auto: { type: Number, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_merged_type_declarations() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-merged-type-declarations-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export interface Foo { a: string }\n",
                "export interface Foo { b: number }\n",
                "export namespace Bar { export type A = string }\n",
                "export namespace Bar { export type B = number }\n",
                "export namespace Baz { export type A = string }\n",
                "export interface Baz { b: number }\n",
                "export enum Kind { A = 1 }\n",
                "export enum Kind { B = 'hi' }\n",
                "export type Props = { ",
                "foo: Foo['a'], ",
                "bar: Foo['b'], ",
                "nsA: Bar.A, ",
                "nsB: Bar.B, ",
                "mixedNs: Baz.A, ",
                "mixedInterface: Baz['b'], ",
                "kind: Kind ",
                "}\n",
                "export type ModelValue = Kind"
            ),
        )
        .expect("write merged types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "const props = defineProps<Props>()\n",
                    "const model = defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["types.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("bar: { type: Number, required: true }"));
        assert!(content.contains("nsA: { type: String, required: true }"));
        assert!(content.contains("nsB: { type: Number, required: true }"));
        assert!(content.contains("mixedNs: { type: String, required: true }"));
        assert!(content.contains("mixedInterface: { type: Number, required: true }"));
        assert!(content.contains("kind: { type: [Number, String], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_bare_package_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let node_modules = dir.join("node_modules");
        let types_pkg = node_modules.join("vuec-bridge-types");
        let types_dist = types_pkg.join("dist");
        std::fs::create_dir_all(&types_dist).expect("create types package");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"}}"#,
        )
        .expect("write package map config");
        std::fs::write(
            types_pkg.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write types package manifest");
        std::fs::write(
            types_dist.join("index.d.ts"),
            "export interface Props { root: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write types package root");
        std::fs::write(
            types_dist.join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");

        let exports_pkg = node_modules.join("vuec-bridge-exports");
        std::fs::create_dir_all(exports_pkg.join("types")).expect("create exports package");
        std::fs::write(
            exports_pkg.join("package.json"),
            r#"{"exports":{"./feature":{"types":"./types/feature.d.ts","default":"./dist/feature.js"}}}"#,
        )
        .expect("write exports package manifest");
        std::fs::write(
            exports_pkg.join("types").join("feature.d.ts"),
            "export type FeatureProps = { count?: number }",
        )
        .expect("write feature type");

        let ambient_pkg = node_modules.join("@types").join("vuec-bridge-ambient");
        std::fs::create_dir_all(&ambient_pkg).expect("create @types package");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambient: string }",
        )
        .expect("write ambient type");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from 'vuec-bridge-types'\n",
                    "import type { FeatureProps } from 'vuec-bridge-exports/feature'\n",
                    "import type { AmbientProps } from 'vuec-bridge-ambient'\n",
                    "const props = defineProps<Props & FeatureProps & AmbientProps>()\n",
                    "const model = defineModel<import('vuec-bridge-types').ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            types_dist.join("index.d.ts"),
            types_dist.join("model.d.ts"),
            exports_pkg.join("types").join("feature.d.ts"),
            ambient_pkg.join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: false }"));
        assert!(content.contains("ambient: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_package_types_versions_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-types-versions-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let node_modules = dir.join("node_modules");
        let versioned_pkg = node_modules.join("vuec-bridge-typesversions");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("future").join("feature"))
            .expect("create future types");
        std::fs::create_dir_all(versioned_pkg.join("ts5").join("feature"))
            .expect("create ts5 types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["future/index.d.ts"],
                        "feature/*": ["future/feature/*.d.ts"]
                    },
                    "5.* || ^4.8": {
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
            "export interface Props { fallbackRoot: string }",
        )
        .expect("write fallback root types");
        std::fs::write(
            versioned_pkg.join("future").join("index.d.ts"),
            "export interface Props { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
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
            "export interface Props { root: string }\nexport type ModelValue = import('./model').ModelValue",
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
            .join("vuec-bridge-typesversions-ambient");
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

        let type_root_pkg = dir.join("typings").join("versioned-global");
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

        let ordered_pkg = node_modules.join("vuec-bridge-typesversions-ordered");
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

        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "files": [],
                "compilerOptions": {
                    "types": ["versioned-global"],
                    "typeRoots": ["./typings"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from 'vuec-bridge-typesversions'\n",
                    "import type { FeatureProps } from 'vuec-bridge-typesversions/feature/item'\n",
                    "import type { AmbientProps } from 'vuec-bridge-typesversions-ambient'\n",
                    "import type { OrderedProps } from 'vuec-bridge-typesversions-ordered'\n",
                    "defineProps<Props & FeatureProps & AmbientProps & TypeRootGlobalProps & OrderedProps>()\n",
                    "defineModel<import('vuec-bridge-typesversions').ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("feature: { type: Number, required: false }"));
        assert!(content.contains("ambient: { type: Boolean, required: true }"));
        assert!(content.contains("typeRoot: { type: String, required: true }"));
        assert!(content.contains("orderedFirst: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!content.contains("fallbackRoot"));
        assert!(!content.contains("futureRoot"));
        assert!(!content.contains("futureFeature"));
        assert!(!content.contains("ambientFallback"));
        assert!(!content.contains("typeRootFallback"));
        assert!(!content.contains("orderedSecond"));
        assert!(!content.contains("orderedFallback"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts5").join("index.d.ts"),
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts5").join("model.d.ts"),
            ambient_pkg.join("ts5").join("index.d.ts"),
            type_root_pkg.join("ts5").join("index.d.ts"),
            ordered_pkg.join("first").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_package_types_versions_from_project_typescript() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-types-versions-project-ts-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let node_modules = dir.join("node_modules");
        let typescript_pkg = node_modules.join("typescript");
        std::fs::create_dir_all(&typescript_pkg).expect("create typescript package");
        std::fs::write(
            typescript_pkg.join("package.json"),
            r#"{"version":"5.2.0"}"#,
        )
        .expect("write typescript manifest");

        let versioned_pkg = node_modules.join("vuec-bridge-typesversions-project-ts");
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

        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from 'vuec-bridge-typesversions-project-ts'\n",
                    "import type { FeatureProps } from 'vuec-bridge-typesversions-project-ts/feature/item'\n",
                    "defineProps<Props & FeatureProps>()\n",
                    "defineModel<import('vuec-bridge-typesversions-project-ts').ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("futureRoot: { type: String, required: true }"));
        assert!(content.contains("futureFeature: { type: Number, required: false }"));
        assert!(content.contains("\"modelValue\": { type: Number },"));
        assert!(!content.contains("baselineRoot"));
        assert!(!content.contains("baselineFeature"));
        assert!(!content.contains("legacyRoot"));
        assert!(!content.contains("legacyFeature"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts52").join("index.d.ts"),
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts52").join("model.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }
