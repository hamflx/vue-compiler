    #[test]
    fn vue3_compile_script_resolves_parameter_tuple_utility_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Fn = (value: string, count: number, active?: boolean) => void
type Ctor = new (name: string, flags: boolean[]) => object
type Props = {
  first: Parameters<Fn>[0]
  anyParam: Parameters<Fn>[number]
  ctorFirst: ConstructorParameters<Ctor>[0]
  ctorAny: ConstructorParameters<Ctor>[number]
  inlineParam: Parameters<(files: File[], done: () => void) => void>[number]
}
defineProps<Props>()
defineModel<Parameters<Fn>[number] | ConstructorParameters<Ctor>[number]>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("first: { type: String, required: true }"));
        assert!(script
            .content
            .contains("anyParam: { type: [String, Number, Boolean], required: true }"));
        assert!(script
            .content
            .contains("ctorFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("ctorAny: { type: [String, Array], required: true }"));
        assert!(script
            .content
            .contains("inlineParam: { type: [Array, Function], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean, Array] },"));
        assert_eq!(
            script.bindings.get("first").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("ctorAny").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_signature_parameter_tuple_utility_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Callable = {
  (value: string, count: number): void
  (active: boolean): void
}
interface InterfaceCallable {
  (name: string, flags: boolean[]): void
}
type Newable = {
  new (id: number, done: () => void): object
}
interface InterfaceNewable {
  new (label: string, enabled: boolean): object
}
type Props = {
  callAny: Parameters<Callable>[number]
  callFirst: Parameters<InterfaceCallable>[0]
  newAny: ConstructorParameters<Newable>[number]
  newSecond: ConstructorParameters<InterfaceNewable>[1]
}
defineProps<Props>()
defineModel<Parameters<Callable>[number] | ConstructorParameters<InterfaceNewable>[number]>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("callAny: { type: [String, Boolean, Number], required: true }"));
        assert!(script
            .content
            .contains("callFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("newAny: { type: [Number, Function], required: true }"));
        assert!(script
            .content
            .contains("newSecond: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(
            script.bindings.get("callFirst").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("newSecond").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_interface_extends_signature_parameter_tuple_utility_runtime_types(
    ) {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Callable extends BaseCallable {
  (active: boolean): void
}
interface BaseCallable {
  (name: string, count: number): void
}
interface Newable extends BaseNewable {
  new (label: string): object
}
interface BaseNewable {
  new (id: number, done: () => void): object
}
type Props = {
  callAny: Parameters<Callable>[number]
  callSecond: Parameters<Callable>[1]
  newAny: ConstructorParameters<Newable>[number]
  newSecond: ConstructorParameters<Newable>[1]
}
defineProps<Props>()
defineModel<Parameters<Callable>[number] | ConstructorParameters<Newable>[number]>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("callAny: { type: [Boolean, String, Number], required: true }"));
        assert!(script
            .content
            .contains("callSecond: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("newAny: { type: [String, Number, Function], required: true }"));
        assert!(script
            .content
            .contains("newSecond: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String, Number, Function] },"));
        assert_eq!(
            script.bindings.get("callAny").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("newSecond").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_return_type_utility_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare function makeLabel(): string
declare const makeCount: () => number
type BooleanFactory = () => boolean
type Callable = {
  (value: string): Date
  (value: number): Error
}
interface InterfaceFactory {
  (active: boolean): string[]
}
interface ExtendedFactory extends InterfaceFactory {
  (value: number): boolean
}
type Props = {
  label: ReturnType<typeof makeLabel>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<BooleanFactory>
  mixed: ReturnType<Callable>
  list: ReturnType<InterfaceFactory>
  extended: ReturnType<ExtendedFactory>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeLabel> | ReturnType<BooleanFactory>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("mixed: { type: [Date, Error], required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("extended: { type: [Boolean, Array], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean] },"));
        assert_eq!(
            script.bindings.get("label").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("extended").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_external_default_function_return_type_runtime_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("named.ts"),
            "export default function makeDefault(): string { return '' }\nexport function makeCount(): number { return 1 }",
        )
        .expect("write named default function type");
        std::fs::write(
            dir.path().join("anonymous.ts"),
            "export default function(): boolean { return true }",
        )
        .expect("write anonymous default function type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import makeDefault, { makeCount } from './named'
import makeFlag from './anonymous'
type Props = {
  label: ReturnType<typeof makeDefault>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<typeof makeFlag>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeDefault> | ReturnType<typeof makeCount> | ReturnType<typeof makeFlag>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["named.ts", "anonymous.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_external_function_value_return_type_runtime_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("factories.ts"),
            concat!(
                "export type Label = string\n",
                "export type Count = number\n",
                "export type Flag = boolean\n",
                "export const makeLabel = (): Label => ''\n",
                "export const makeCount: () => Count = () => 1\n",
                "export const makeFlag = function(): Flag { return true }"
            ),
        )
        .expect("write function value factories");
        std::fs::write(
            dir.path().join("arrow-default.ts"),
            "export default ((): Date => new Date())",
        )
        .expect("write default arrow function value");
        std::fs::write(
            dir.path().join("function-default.ts"),
            "export default (function(): Error { return new Error() })",
        )
        .expect("write default function expression value");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import makeDate from './arrow-default'
import makeError from './function-default'
import { makeLabel, makeCount, makeFlag } from './factories'
type Props = {
  label: ReturnType<typeof makeLabel>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<typeof makeFlag>
  date: ReturnType<typeof makeDate>
  error: ReturnType<typeof makeError>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeLabel> | ReturnType<typeof makeCount> | ReturnType<typeof makeFlag> | ReturnType<typeof makeDate> | ReturnType<typeof makeError>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("date: { type: Date, required: true }"));
        assert!(script
            .content
            .contains("error: { type: Error, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean, Date, Error] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["factories.ts", "arrow-default.ts", "function-default.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_unannotated_function_return_runtime_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("factories.ts"),
            concat!(
                "export function makeLabel() { return 'label' }\n",
                "export const makeCount = () => 1\n",
                "export const makeFlag = function() { return true }\n",
                "export const makeList = () => []\n",
                "export function makeBox() { return { label: 'box' } }"
            ),
        )
        .expect("write unannotated factories");
        std::fs::write(
            dir.path().join("date.ts"),
            "export default function makeDate() { return new Date() }",
        )
        .expect("write default unannotated function");
        std::fs::write(
            dir.path().join("error.ts"),
            "export default (function() { return new Error('x') })",
        )
        .expect("write default unannotated function expression");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import makeDate from './date'
import makeError from './error'
import { makeLabel, makeCount, makeFlag, makeList, makeBox } from './factories'
type Props = {
  label: ReturnType<typeof makeLabel>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<typeof makeFlag>
  list: ReturnType<typeof makeList>
  box: ReturnType<typeof makeBox>
  made: ReturnType<typeof import('./factories').makeFlag>
  created: ReturnType<typeof makeDate>
  error: ReturnType<typeof makeError>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeLabel> | ReturnType<typeof makeCount> | ReturnType<typeof makeFlag> | ReturnType<typeof makeList> | ReturnType<typeof makeBox> | ReturnType<typeof makeDate> | ReturnType<typeof makeError>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("box: { type: Object, required: true }"));
        assert!(script
            .content
            .contains("made: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("created: { type: Date, required: true }"));
        assert!(script
            .content
            .contains("error: { type: Error, required: true }"));
        assert!(script.content.contains(
            "\"modelValue\": { type: [String, Number, Boolean, Array, Object, Date, Error] },"
        ));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["factories.ts", "date.ts", "error.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }
