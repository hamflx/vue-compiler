    #[test]
    fn vue3_compile_script_resolves_type_query_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare const text: string
declare const count: number
declare const flag: boolean
declare const boxed: { id: string }
declare const list: string[]
type Props = {
  text: typeof text
  count: typeof count
  flag: typeof flag
  boxed: typeof boxed
  list: typeof list
  keys: keyof typeof boxed
}
defineProps<Props>()
defineModel<typeof flag | typeof list>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("text: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("boxed: { type: Object, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("keys: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, Array] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_qualified_type_query_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare namespace Values {
  export declare const text: string
  export declare const boxed: { id: string }
  export declare const list: string[]
}
type Props = {
  text: typeof Values.text
  keys: keyof typeof Values.boxed
  list: typeof Values.list
}
defineProps<Props>()
defineModel<typeof Values.text | typeof Values.list>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("text: { type: String, required: true }"));
        assert!(script
            .content
            .contains("keys: { type: String, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Array] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_import_type_query_runtime_types_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let values_file = dir.path().join("values.ts");
        std::fs::write(
            &values_file,
            concat!(
                "export declare const text: string\n",
                "export declare const boxed: { id: string }\n",
                "export declare const list: string[]\n",
                "export declare const options: { enabled: BooleanConstructor }\n",
                "export function make(): boolean { return true }\n"
            ),
        )
        .expect("write type query values");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
type Props =
  ExtractPropTypes<typeof import('./values').options> & {
    text: typeof import('./values').text
    keys: keyof typeof import('./values').boxed
    list: typeof import('./values').list
    made: ReturnType<typeof import('./values').make>
  }
defineProps<Props>()
defineModel<typeof import('./values').text | ReturnType<typeof import('./values').make> | typeof import('./values').list>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("enabled: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("text: { type: String, required: true }"));
        assert!(script
            .content
            .contains("keys: { type: String, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("made: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean, Array] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [values_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_signature_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Callable = { (): string }
type Constructable = { new (): object }
type Mixed = { (): string; value: number }
interface InterfaceCallable {
  (): string
}
interface InterfaceMixed {
  new (): object
  value: number
}
type Props = {
  call: Callable
  ctor: Constructable
  mixed: Mixed
  ifaceCall: InterfaceCallable
  ifaceMixed: InterfaceMixed
}
defineProps<Props>()
defineModel<Callable | InterfaceMixed>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("call: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("ctor: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("mixed: { type: [Function, Object], required: true }"));
        assert!(script
            .content
            .contains("ifaceCall: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("ifaceMixed: { type: [Function, Object], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Function, Object] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_intersection_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Callable = { (): string }
type Box = { value: number }
type UnknownOnly = any
type Props = {
  scalar: string & number
  callableBox: Callable & Box
  maybe: any | boolean
  unknown: UnknownOnly
}
defineProps<Props>()
defineModel<(string & number) | (Callable & Box)>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("scalar: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("callableBox: { type: [Function, Object], required: true }"));
        assert!(script
            .content
            .contains("maybe: { type: Boolean, required: true, skipCheck: true }"));
        assert!(script
            .content
            .contains("unknown: { type: null, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Function, Object] },"));
        assert!(!script.content.contains("type: Unknown"));
        assert!(!script.content.contains("[Unknown"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }
