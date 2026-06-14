    #[test]
    fn vue3_compile_script_resolves_merged_type_declarations() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Foo {
  a: string
}
interface Foo {
  b: number
}
namespace Bar {
  export type A = string
}
namespace Bar {
  export type B = number
}
namespace Baz {
  export type A = string
}
interface Baz {
  b: number
}
enum Kind {
  A = 1
}
enum Kind {
  B = 'hi'
}
type Props = {
  foo: Foo['a']
  bar: Foo['b']
  nsA: Bar.A
  nsB: Bar.B
  mixedNs: Baz.A
  mixedInterface: Baz['b']
  kind: Kind
}
defineProps<Props>()
defineModel<Kind>()
</script>"#,
        );
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
            .contains("nsA: { type: String, required: true }"));
        assert!(script
            .content
            .contains("nsB: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("mixedNs: { type: String, required: true }"));
        assert!(script
            .content
            .contains("mixedInterface: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("kind: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(
            script.bindings.get("mixedInterface").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_reports_props_type_resolution_errors() {
        let mut compiler = SfcCompiler::new();

        let unresolved = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
defineProps<X>()
</script>"#,
        );
        let script = compiler.compile_script(&unresolved, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("Unresolvable type reference or unsupported built-in utility type")
        }));

        let computed = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
defineProps<{ [Foo]: string }>()
</script>"#,
        );
        let script = compiler.compile_script(&computed, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("Unsupported computed key in type referenced by a macro")
        }));

        let indexed = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
defineProps<X[K]>()
</script>"#,
        );
        let script = compiler.compile_script(&indexed, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Unsupported type when resolving index type")));

        let missing_import = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
import { X } from './foo'
defineProps<X>()
</script>"#,
        );
        let script = compiler.compile_script(&missing_import, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Failed to resolve import source \"./foo\".")));

        let member_runtime = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
import type P from 'unknown'
defineProps<{ foo: T, bar: T['bar'], baz: P }>()
</script>"#,
        );
        let script = compiler.compile_script(&member_runtime, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("bar").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_resolves_top_level_indexed_access_props_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type T = { bar: number }
type S = { nested: { foo: T['bar'] } }
defineProps<S['nested']>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_resolves_runtime_utility_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type MaybeText = string | null
type Props = {
  label: NonNullable<MaybeText>
  extracted: Extract<string | number | boolean, number | boolean>
  excluded: Exclude<string | number, number>
}
defineProps<Props>()
defineModel<NonNullable<string | null> | Extract<number | boolean, boolean> | Exclude<string | number, number>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("extracted: { type: [Number, Boolean], required: true }"));
        assert!(script
            .content
            .contains("excluded: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(
            script.bindings.get("label").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("extracted").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_static_conditional_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Runtime<T> = T extends 'text' ? string : T extends 'count' ? number : boolean
type Props = {
  directTrue: 'on' extends 'on' ? boolean : string
  directFalse: 'off' extends 'on' ? boolean : string
  text: Runtime<'text'>
  count: Runtime<'count'>
  active: Runtime<'active'>
  unresolved: Runtime<'text' | 'count'>
}
defineProps<Props>()
defineModel<Runtime<'text'> | Runtime<'count'> | Runtime<'active'>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("directTrue: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("directFalse: { type: String, required: true }"));
        assert!(script
            .content
            .contains("text: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("active: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("unresolved: { type: null, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean] },"));
        assert_eq!(
            script.bindings.get("text").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_bigint_literal_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Big = 1n
type Props = {
  literal: 1n
  union: 1n | 'text'
  alias: Big
  keyword: bigint
}
defineProps<Props>()
defineModel<1n | 'text'>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("literal: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("union: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("alias: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("keyword: { type: null, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(
            script.bindings.get("literal").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_builtin_wrapper_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Props = {
  list: ReadonlyArray<string>
  params: Parameters<(value: string) => void>
  ctorParams: ConstructorParameters<any>
  map: ReadonlyMap<string, number>
  set: ReadonlySet<string>
  err: Error
  loud: Uppercase<'foo'>
  maybe: MaybeRef<string[]>
  getter: MaybeRefOrGetter<boolean>
  ref: Ref<number>
}
defineProps<Props>()
defineModel<ReadonlyArray<string> | ReadonlyMap<string, number> | ReadonlySet<string> | Error | MaybeRefOrGetter<boolean> | Parameters<() => void> | Uppercase<'foo'>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("params: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("ctorParams: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("map: { type: Map, required: true }"));
        assert!(script
            .content
            .contains("set: { type: Set, required: true }"));
        assert!(script
            .content
            .contains("err: { type: Error, required: true }"));
        assert!(script
            .content
            .contains("loud: { type: String, required: true }"));
        assert!(script
            .content
            .contains("maybe: { type: [Object, Array], required: true }"));
        assert!(script
            .content
            .contains("getter: { type: [Object, Function, Boolean], required: true }"));
        assert!(script
            .content
            .contains("ref: { type: Object, required: true }"));
        assert!(script.content.contains(
            "\"modelValue\": { type: [Array, Map, Set, Error, Object, Function, Boolean, String] },"
        ));
        assert_eq!(
            script.bindings.get("getter").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_mapped_identity_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type RuntimeMirror<T> = { [K in keyof T]: T[K] }
type Props = {
  label: RuntimeMirror<string | number>
  boxed: RuntimeMirror<{ value: boolean }>
  list: RuntimeMirror<ReadonlyArray<string>>
}
defineProps<Props>()
defineModel<RuntimeMirror<string | boolean>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("boxed: { type: Object, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_type_operator_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Base = {
  name: string
  1: boolean
}
type Props = {
  readonlyList: readonly string[]
  objectKeys: keyof Base
  literalKeys: keyof { [index: number]: string; label: string }
  arrayKeys: keyof ReadonlyArray<string>
  anyKeys: keyof any
  pickedKeys: keyof Pick<Base, 'name'>
}
defineProps<Props>()
defineModel<readonly boolean[] | keyof any>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("readonlyList: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("objectKeys: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("literalKeys: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("arrayKeys: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("anyKeys: { type: [String, Number, Symbol], required: true }"));
        assert!(script
            .content
            .contains("pickedKeys: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Array, String, Number, Symbol] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }
