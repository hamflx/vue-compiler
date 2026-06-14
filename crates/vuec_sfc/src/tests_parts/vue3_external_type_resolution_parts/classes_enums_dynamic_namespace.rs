    #[test]
    fn vue3_compile_script_resolves_class_declaration_types_and_default_class_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("classes.ts"),
            "export class NamedClass {}\nexport type Props = { named: NamedClass }\nexport default class DefaultClass {}",
        )
        .expect("write class types");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export default class LeafClass {}",
        )
        .expect("write default class leaf");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { default } from './leaf'",
        )
        .expect("write default class facade");
        std::fs::write(
            dir.path().join("named_facade.ts"),
            "export { NamedClass as RenamedClass } from './classes'",
        )
        .expect("write named class facade");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            "declare type GlobalProps = { global: GlobalClass }\ndeclare class GlobalClass {}",
        )
        .expect("write global class types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import DefaultClass from './facade'
import { NamedClass, Props } from './classes'
import { RenamedClass } from './named_facade'
type LocalProps = { local: LocalClass, defaulted: DefaultClass, named: NamedClass, renamed: RenamedClass, props: Props }
class LocalClass {}
const props = defineProps<LocalProps & GlobalProps>()
const model = defineModel<DefaultClass>()
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
        for prop in ["local", "defaulted", "named", "renamed", "props", "global"] {
            assert!(
                script
                    .content
                    .contains(&format!("{prop}: {{ type: Object, required: true }}")),
                "{}",
                script.content
            );
        }
        assert!(script.content.contains("\"modelValue\": { type: Object },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("classes.ts"),
            dir.path().join("leaf.ts"),
            dir.path().join("facade.ts"),
            dir.path().join("named_facade.ts"),
            global,
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_enum_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("enums.ts"),
            "export enum Kind { A = 'a', B = 'b' }\nexport enum Code { A = 1, B = 2 }\nexport enum Mixed { A = 'a', B = 1 }\nexport enum Auto { A, B }\nexport type Props = { kind: Kind, code?: Code, mixed: Mixed, auto: Auto }\nexport type ModelValue = Kind | Code",
        )
        .expect("write enum types");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { Props as FacadeProps, ModelValue as FacadeModel } from './enums'",
        )
        .expect("write enum facade");
        std::fs::write(
            dir.path().join("namespace.ts"),
            "export namespace Nested { export enum Flag { Yes = 'yes', No = 'no' } export type Props = { flag: Flag } }",
        )
        .expect("write namespace enum types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { FacadeProps, FacadeModel } from './facade'
import * as Ns from './namespace'
const props = defineProps<FacadeProps & Ns.Nested.Props>()
const model = defineModel<FacadeModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("kind: { type: String, required: true }"));
        assert!(script
            .content
            .contains("code: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("mixed: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("auto: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["enums.ts", "facade.ts", "namespace.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_dynamic_import_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export type Props = { foo: string, bar: import('./bar').N }",
        )
        .expect("write props type");
        std::fs::write(dir.path().join("bar.ts"), "export type N = number")
            .expect("write prop leaf type");
        std::fs::write(
            dir.path().join("events.ts"),
            "export type Events = import('./event_leaf').Events",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("event_leaf.ts"),
            "export type Events = { (e: 'save'): void }",
        )
        .expect("write event leaf type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = import('./model_leaf').ModelValue",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("model_leaf.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model leaf type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
const props = defineProps<import('./foo').Props>()
const emit = defineEmits<import('./events').Events>()
const model = defineModel<import('./model').ModelValue>()
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
            .contains("bar: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "foo.ts",
            "bar.ts",
            "events.ts",
            "event_leaf.ts",
            "model.ts",
            "model_leaf.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_namespace_imported_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("types.ts"),
            "export type Props = { foo: string }\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = boolean | string\nexport type Unused = { nope: string }",
        )
        .expect("write namespace types");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export namespace Nested { export type ExtraProps = { count?: number } }",
        )
        .expect("write nested namespace types");
        std::fs::write(
            dir.path().join("dynamic.ts"),
            "export namespace Types { export type Props = { bar: number } }",
        )
        .expect("write dynamic namespace types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import * as Types from './types'
import * as Leaf from './leaf'
const props = defineProps<Types.Props & Leaf.Nested.ExtraProps & import('./dynamic').Types.Props>()
const emit = defineEmits<Types.Events>()
const model = defineModel<Types.ModelValue>()
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
            .contains("bar: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["types.ts", "leaf.ts", "dynamic.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("unused") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_vue_type_imports_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.vue"),
            "<template><div /></template><script lang=\"ts\">export type Props = { foo: number }</script>",
        )
        .expect("write foo vue");
        std::fs::write(
            dir.path().join("bar.vue"),
            "<script setup lang=\"tsx\">export type ExtraProps = { bar: string }</script>",
        )
        .expect("write bar vue");
        std::fs::write(
            dir.path().join("events.vue"),
            "<script setup lang=\"ts\">export type Events = { (e: 'save'): void }</script>",
        )
        .expect("write events vue");
        std::fs::write(
            dir.path().join("model.vue"),
            "<script lang=\"ts\">export type ModelValue = boolean | string</script>",
        )
        .expect("write model vue");
        std::fs::write(
            dir.path().join("leaf.vue"),
            "<script setup lang=\"ts\">export type LeafProps = { leaf?: boolean }</script>",
        )
        .expect("write leaf vue");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { LeafProps } from './leaf.vue'",
        )
        .expect("write facade");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { Props } from './foo.vue'
import { ExtraProps } from './bar.vue'
import { LeafProps } from './facade'
import { Events } from './events.vue'
import { ModelValue } from './model.vue'
const props = defineProps<Props & ExtraProps & LeafProps>()
const emit = defineEmits<Events>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: String, required: true }"));
        assert!(script
            .content
            .contains("leaf: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "foo.vue",
            "bar.vue",
            "facade.ts",
            "leaf.vue",
            "events.vue",
            "model.vue",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_arbitrary_extension_type_sidecars() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.d.vue.ts"),
            "export type FooProps = { foo: number }",
        )
        .expect("write vue sidecar");
        std::fs::write(dir.path().join("foo.vue"), "<template><div /></template>")
            .expect("write foo vue");
        std::fs::write(
            dir.path().join("bar.d.css.ts"),
            "export type BarProps = { bar: string }",
        )
        .expect("write css sidecar");
        std::fs::write(dir.path().join("bar.css"), ".bar { color: red; }").expect("write css");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { FooProps } from './foo.vue'
import { BarProps } from './bar.css'
defineProps<FooProps & BarProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: String, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["foo.d.vue.ts", "bar.d.css.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }
