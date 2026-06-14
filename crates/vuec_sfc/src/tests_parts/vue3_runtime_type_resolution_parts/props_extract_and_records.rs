    #[test]
    fn vue3_compile_script_resolves_extract_prop_types_from_declared_const_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare const props: {
  foo: StringConstructor
  bar: { type: import('foo').EpPropFinalized<BooleanConstructor>, required: true }
}
type Props = ExtractPropTypes<typeof props>
const resolved = defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("bar: { type: Boolean, required: true }"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("bar").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_extract_prop_types_from_runtime_props_object() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("user.ts"),
            "export interface User { id: string }",
        )
        .expect("write user type");
        std::fs::write(
            dir.path().join("props.ts"),
            concat!(
                "import type { PropType } from 'vue'\n",
                "import type { User } from './user'\n",
                "export const props = {\n",
                "  name: String,\n",
                "  active: { type: Boolean, required: true },\n",
                "  score: { type: [Number, String] },\n",
                "  user: Object as PropType<User>\n",
                "}\n"
            ),
        )
        .expect("write runtime props");
        std::fs::write(
            dir.path().join("default-props.ts"),
            concat!(
                "const props = {\n",
                "  flag: Boolean,\n",
                "  created: { type: Date, default: () => new Date() }\n",
                "}\n",
                "export { props as default }\n"
            ),
        )
        .expect("write default runtime props");
        std::fs::write(
            dir.path().join("direct-default-props.ts"),
            concat!(
                "import type { PropType } from 'vue'\n",
                "import type { User } from './user'\n",
                "export default {\n",
                "  direct: { type: String, required: true },\n",
                "  owner: Object as PropType<User>,\n",
                "  mode: { type: [Boolean, Number] }\n",
                "}\n"
            ),
        )
        .expect("write direct default runtime props");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { props as namedProps } from './props'
import defaultProps from './default-props'
import directDefaultProps from './direct-default-props'
type Props =
  ExtractPropTypes<typeof namedProps> &
  Partial<ExtractPropTypes<typeof defaultProps>> &
  ExtractPropTypes<typeof directDefaultProps>
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("name: { type: String, required: false }"));
        assert!(script
            .content
            .contains("active: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("score: { type: [Number, String], required: false }"));
        assert!(script
            .content
            .contains("user: { type: Object, required: false }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("created: { type: Date, required: false }"));
        assert!(script
            .content
            .contains("direct: { type: String, required: true }"));
        assert!(script
            .content
            .contains("owner: { type: Object, required: false }"));
        assert!(script
            .content
            .contains("mode: { type: [Boolean, Number], required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "user.ts",
            "props.ts",
            "default-props.ts",
            "direct-default-props.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_partial_import_extract_prop_types_return_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare const props: () => {
  foo: StringConstructor
  active: { type: BooleanConstructor, required: true }
}
type Props = Partial<import('vue').ExtractPropTypes<ReturnType<typeof props>>>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("active: { type: Boolean, required: false }"));
        assert_eq!(
            script.bindings.get("active").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_pick_omit_utility_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Base = { foo: number, bar: string, baz?: boolean }
type Keys = 'foo' | 'bar'
type Props = Pick<Base, Keys> & Partial<Omit<Base, Keys>>
defineProps<Props>()
</script>"#,
        );
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
            .contains("baz: { type: Boolean, required: false }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_merges_duplicate_union_intersection_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Left = { shared: string; left?: boolean }
type Right = { shared?: number; right: Function }
type Props = Left & Right & ({ variant: string } | { variant?: boolean })
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(script.content.matches("shared: {").count(), 1);
        assert_eq!(script.content.matches("variant: {").count(), 1);
        assert!(script
            .content
            .contains("shared: { type: [String, Number], required: false }"));
        assert!(script
            .content
            .contains("left: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("right: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("variant: { type: [String, Boolean], required: false }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_generic_props_type_aliases() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Foo = { foo: string }
type Bar = { bar?: number }
type Props<T, U> = Readonly<T & U & { baz: boolean }>
type Box<T> = { value: T }
type Optional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>
interface Test { one: string; two: string }
defineProps<Props<Foo, Bar> & Box<string> & Optional<Test, 'one'> & Pick<Test, keyof Test>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("baz: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("value: { type: String, required: true }"));
        assert!(script
            .content
            .contains("one: { type: String, required: false }"));
        assert!(script
            .content
            .contains("two: { type: String, required: true }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_mapped_template_literal_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Flag = 'foo' | 'bar'
type Breakpoints = 'sm' | 'md' | 'lg'
type BreakpointFactory<T extends string, V> = {
  [K in Breakpoints as `${T}${Capitalize<K>}`]?: V
}
type Props = {
  [K in `_${Flag}_${Breakpoints}_`]: string
} & {
  [K in Flag as `${K}_flag`]: boolean
} & {
  [K in Uppercase<Extract<Flag, 'foo'>> as `upper${Capitalize<Lowercase<K>>}`]?: string
} & {
  [K in Lowercase<'LOUD'> as `${K}_lower`]: number
} & {
  [K in Uncapitalize<'Title'> as `${K}_name`]: boolean
} & {
  [K in Exclude<Flag, 'bar'> as `${K}_only`]: string
} & BreakpointFactory<'cols', number>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("_foo_sm_: { type: String, required: true }"));
        assert!(script
            .content
            .contains("_foo_md_: { type: String, required: true }"));
        assert!(script
            .content
            .contains("_bar_lg_: { type: String, required: true }"));
        assert!(script
            .content
            .contains("foo_flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("bar_flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("upperFoo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("loud_lower: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("title_name: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("foo_only: { type: String, required: true }"));
        assert!(script
            .content
            .contains("colsSm: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("colsMd: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("colsLg: { type: Number, required: false }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_record_props_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Flag = 'foo' | 'bar'
type Breakpoints = 'sm' | 'md'
type Props =
  Record<`${Flag}_${Breakpoints}`, number> &
  Partial<Record<Uppercase<Extract<Flag, 'foo'>>, string>> &
  Record<Exclude<Flag, 'bar'>, boolean>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo_sm: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("foo_md: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar_sm: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar_md: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("FOO: { type: String, required: false }"));
        assert!(script
            .content
            .contains("foo: { type: Boolean, required: true }"));
        assert_eq!(
            script.bindings.get("foo_sm").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("FOO").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_indexed_access_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Base = {
  name: string
  count?: number
  active: boolean
  method(): void
  run: () => void
}
type A = (string | number)[]
type AA = Array<string>
type T = [1, 'foo']
type TT = [foo: 1, bar: 'foo']
type ValueOf<T, K extends keyof T> = T[K]
type Props = {
  directMethod(): void
  label: Base['name']
  scalar: Base['name' | 'count']
  method: Base['method']
  callable: Base['run']
  methodOrCallable: Base['method'] | Base['run']
  methodOrLabel: Base['method'] | Base['name']
  generic: ValueOf<Base, 'active'>
  arrayItem: A[number]
  genericArrayItem: AA[number]
  tupleItem: T[number]
  namedTupleItem: TT[number]
}
defineProps<Props>()
defineModel<A[number] | TT[number] | Base['method'] | Base['run']>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("directMethod: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("scalar: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("method: { type: null, required: true }"));
        assert!(script
            .content
            .contains("callable: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("methodOrCallable: { type: Function, required: true, skipCheck: true }"));
        assert!(script
            .content
            .contains("methodOrLabel: { type: null, required: true }"));
        assert!(script
            .content
            .contains("generic: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("arrayItem: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("genericArrayItem: { type: String, required: true }"));
        assert!(script
            .content
            .contains("tupleItem: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("namedTupleItem: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Function], skipCheck: true },"));
        assert_eq!(
            script.bindings.get("label").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("generic").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }
