    #[test]
    fn vue3_sfc_bridge_compile_script_merges_external_duplicate_union_intersection_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-duplicate-union-intersection-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Left = { shared: string; unknownBool: any; left?: boolean }\n",
                "export type Right = { shared?: number; unknownBool: boolean; right: Function }\n",
                "export type Props = Left & Right & ",
                "({ variant: string } | { variant?: boolean }) & ",
                "({ maybe: any } | { maybe: boolean })"
            ),
        )
        .expect("write duplicate props types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert_eq!(content.matches("shared: {").count(), 1);
        assert_eq!(content.matches("variant: {").count(), 1);
        assert_eq!(content.matches("maybe: {").count(), 1);
        assert!(content.contains("shared: { type: [String, Number], required: false }"));
        assert!(content.contains("unknownBool: { type: Boolean, required: true }"));
        assert!(content.contains("left: { type: Boolean, required: false }"));
        assert!(content.contains("right: { type: Function, required: true }"));
        assert!(content.contains("variant: { type: [String, Boolean], required: false }"));
        assert!(content.contains("maybe: { type: Boolean, required: true, skipCheck: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_interface_extends_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export interface Base { ext?: string }\nexport interface Props extends Base { local: number }",
        )
        .expect("write interface props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ext: { type: String, required: false }"));
        assert!(content.contains("local: { type: Number, required: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_forward_interface_extends_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-forward-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export interface Props extends Base { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write interface props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ext: { type: String, required: false }"));
        assert!(content.contains("local: { type: Number, required: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_failed_interface_extends_and_honors_vue_ignore_deps()
    {
        let unresolved_dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-failed-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&unresolved_dir);
        std::fs::create_dir_all(&unresolved_dir).expect("create temp dir");
        std::fs::write(
            unresolved_dir.join("types.ts"),
            "import type Base from 'unknown'\nexport interface Props extends Base { local: number }",
        )
        .expect("write unresolved interface props");

        let unresolved_filename = unresolved_dir.join("Comp.vue");
        let unresolved = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": unresolved_filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let unresolved_content = unresolved["content"].as_str().unwrap_or_default();
        let unresolved_expected_dep = unresolved_dir
            .join("types.ts")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(unresolved["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error.as_str().is_some_and(|error| {
                    error.contains("Failed to resolve extends base type")
                        && error.contains("@vue-ignore")
                })
            }));
        assert!(unresolved_content.contains("local: { type: Number, required: true }"));
        assert_eq!(unresolved["deps"], json!([unresolved_expected_dep]));
        let _ = std::fs::remove_dir_all(&unresolved_dir);

        let ignored_dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-ignored-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&ignored_dir);
        std::fs::create_dir_all(&ignored_dir).expect("create temp dir");
        std::fs::write(
            ignored_dir.join("types.ts"),
            "interface Base { skipped?: string }\nexport interface Props extends /*@vue-ignore*/ Base { local: number }",
        )
        .expect("write ignored interface props");

        let ignored_filename = ignored_dir.join("Comp.vue");
        let ignored = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": ignored_filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let ignored_content = ignored["content"].as_str().unwrap_or_default();
        let ignored_expected_dep = ignored_dir
            .join("types.ts")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(ignored["errors"].as_array().unwrap().is_empty());
        assert!(ignored_content.contains("local: { type: Number, required: true }"));
        assert!(!ignored_content.contains("skipped: {"));
        assert_eq!(ignored["deps"], json!([ignored_expected_dep]));
        let _ = std::fs::remove_dir_all(&ignored_dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_vue_ignore_on_property_signature_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-property-signature-vue-ignore-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "type Foo = string\nexport interface Props { foo: /* @vue-ignore */ Foo; bar?: Foo }",
        )
        .expect("write ignored property signature type");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: null, required: true }"));
        assert!(content.contains("bar: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_forward_type_alias_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-forward-type-alias-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export type Props = Base & { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write type alias props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("local: { type: Number, required: true }"));
        assert!(content.contains("ext: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_forward_type_alias_intersection_emits_deps()
    {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-forward-type-alias-emits-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("events.ts"),
            "export type Emits = Mid & { (e: 'local'): void }\nexport type Mid = Base & { (e: 'mid'): void }\nexport interface Base { (e: 'base'): void }",
        )
        .expect("write type alias emits");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Emits } from './events'\n",
                    "const emit = defineEmits<Emits>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("events.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("emits: [\"base\", \"mid\", \"local\"],"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_define_emits_property_syntax_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-emits-property-syntax-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("events.ts"),
            "export type Emits = { foo: []; bar: [id: number]; 'foo:bar': [] }",
        )
        .expect("write property emits");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Emits } from './events'\n",
                    "const emit = defineEmits<Emits>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("events.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("emits: [\"foo\", \"bar\", \"foo:bar\"],"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_define_emits_union_function_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-emits-union-function-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("events.ts"),
            concat!(
                "export type BaseEmit = 'change'\n",
                "export type Emit = 'some' | 'emit' | BaseEmit\n",
                "export type Emits = ",
                "((e: 'foo' | 'bar') => void) | ",
                "((e: Emit) => void) | ",
                "((e: 'another', val: string) => void)"
            ),
        )
        .expect("write union emits");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Emits } from './events'\n",
                    "const emit = defineEmits<Emits>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("events.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content
            .contains("emits: [\"foo\", \"bar\", \"some\", \"emit\", \"change\", \"another\"],"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }
