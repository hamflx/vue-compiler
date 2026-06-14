    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_global_type_re_exports_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-global-re-export-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("node_modules").join("pkg").join("dist"))
            .expect("create package");
        std::fs::write(dir.join("base.ts"), "export interface Base { age: number }")
            .expect("write base type");
        std::fs::write(dir.join("types.ts"), "export type Name = string")
            .expect("write helper type");
        std::fs::write(
            dir.join("foo.ts"),
            concat!(
                "import type { Base } from './base'\n",
                "import type { Name } from './types'\n",
                "export interface Foo extends Base { name: Name }"
            ),
        )
        .expect("write foo type");
        std::fs::write(dir.join("bar.ts"), "export interface Bar { bar: boolean }")
            .expect("write bar type");
        std::fs::write(dir.join("baz.ts"), "export interface Baz { baz: string }")
            .expect("write baz type");
        let package_dir = dir.join("node_modules").join("pkg");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write package manifest");
        std::fs::write(
            package_dir.join("dist").join("index.d.ts"),
            "export interface PackageType { value: string }",
        )
        .expect("write package types");
        let global = dir.join("global.d.ts");
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

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<Foo & Bar & Baz & PackageType>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy(),
                "options": {
                    "globalTypeFiles": [global.to_string_lossy()]
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("age: { type: Number, required: true }"));
        assert!(content.contains("name: { type: String, required: true }"));
        assert!(content.contains("bar: { type: Boolean, required: true }"));
        assert!(content.contains("baz: { type: String, required: true }"));
        assert!(content.contains("value: { type: String, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            global,
            dir.join("foo.ts"),
            dir.join("base.ts"),
            dir.join("types.ts"),
            dir.join("bar.ts"),
            dir.join("baz.ts"),
            package_dir.join("dist").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_dynamic_import_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-dynamic-import-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("foo.ts"),
            "export type Props = { foo: string, count: import('./bar').Count }",
        )
        .expect("write props");
        std::fs::write(dir.join("bar.ts"), "export type Count = number").expect("write bar");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<import('./foo').Props>()",
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
        let expected = ["foo.ts", "bar.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_namespace_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-namespace-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export type Props = { foo: string }\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = boolean | string",
        )
        .expect("write namespace types");
        std::fs::write(
            dir.join("leaf.ts"),
            "export namespace Nested { export type ExtraProps = { count?: number } }",
        )
        .expect("write leaf types");
        std::fs::write(
            dir.join("dynamic.ts"),
            "export namespace Types { export type Props = { bar: number } }",
        )
        .expect("write dynamic types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import * as Types from './types'\n",
                    "import * as Leaf from './leaf'\n",
                    "const props = defineProps<Types.Props & Leaf.Nested.ExtraProps & import('./dynamic').Types.Props>()\n",
                    "const emit = defineEmits<Types.Events>()\n",
                    "const model = defineModel<Types.ModelValue>()",
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
        let expected = ["types.ts", "leaf.ts", "dynamic.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: false }"));
        assert!(content.contains("bar: { type: Number, required: true }"));
        assert!(content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_vue_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-vue-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("foo.vue"),
            "<template><div /></template><script lang=\"ts\">export type Props = { foo: string }</script>",
        )
        .expect("write foo vue");
        std::fs::write(
            dir.join("bar.vue"),
            "<script setup lang=\"ts\">export type ExtraProps = { count?: number }</script>",
        )
        .expect("write bar vue");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { Props } from './foo.vue'\n",
                    "import { ExtraProps } from './bar.vue'\n",
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
        let expected = ["foo.vue", "bar.vue"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: false }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }
