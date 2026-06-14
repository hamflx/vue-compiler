
    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_extract_prop_types_return_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-extract-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("upload.ts"),
            concat!(
                "import type { PropType } from 'vue'\n",
                "export interface UploadFile<T> { raw: T }\n",
                "export declare function uploadProps<T>(): {\n",
                "  fileList: { type: PropType<UploadFile<T>[]>, default: UploadFile<T>[] }\n",
                "}\n"
            ),
        )
        .expect("write upload props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { uploadProps } from './upload'\n",
                    "declare const props: () => {\n",
                    "  active: { type: BooleanConstructor, required: true }\n",
                    "}\n",
                    "type Props = Partial<import('vue').ExtractPropTypes<ReturnType<typeof props>>> & import('vue').ExtractPropTypes<ReturnType<typeof uploadProps>>\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("upload.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("active: { type: Boolean, required: false }"));
        assert!(content.contains("fileList: { type: Array, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_runtime_props_object_extract_prop_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-runtime-props-object-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("user.ts"), "export interface User { id: string }")
            .expect("write user type");
        std::fs::write(
            dir.join("props.ts"),
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
            dir.join("default-props.ts"),
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
            dir.join("direct-default-props.ts"),
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

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { props as namedProps } from './props'\n",
                    "import defaultProps from './default-props'\n",
                    "import directDefaultProps from './direct-default-props'\n",
                    "type Props =\n",
                    "  ExtractPropTypes<typeof namedProps> &\n",
                    "  Partial<ExtractPropTypes<typeof defaultProps>> &\n",
                    "  ExtractPropTypes<typeof directDefaultProps>\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_deps = json!([
            dir.join("default-props.ts")
                .to_string_lossy()
                .replace('\\', "/"),
            dir.join("direct-default-props.ts")
                .to_string_lossy()
                .replace('\\', "/"),
            dir.join("props.ts").to_string_lossy().replace('\\', "/"),
            dir.join("user.ts").to_string_lossy().replace('\\', "/")
        ]);
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("name: { type: String, required: false }"));
        assert!(content.contains("active: { type: Boolean, required: true }"));
        assert!(content.contains("score: { type: [Number, String], required: false }"));
        assert!(content.contains("user: { type: Object, required: false }"));
        assert!(content.contains("flag: { type: Boolean, required: false }"));
        assert!(content.contains("created: { type: Date, required: false }"));
        assert!(content.contains("direct: { type: String, required: true }"));
        assert!(content.contains("owner: { type: Object, required: false }"));
        assert!(content.contains("mode: { type: [Boolean, Number], required: false }"));
        assert_eq!(compiled["deps"], expected_deps);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_generic_utility_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-generic-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export type Props<T> = Readonly<Partial<T>>\nexport type Base = { ext: string }",
        )
        .expect("write generic props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, Base } from './types'\n",
                    "defineProps<Props<Base>>()",
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
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_mapped_template_literal_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-mapped-template-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "type Breakpoints = 'sm' | 'md'\n",
                "export type Props<T extends string, V> = {\n",
                "  [K in Breakpoints as `${T}${Capitalize<K>}`]?: V\n",
                "}"
            ),
        )
        .expect("write mapped props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props<'cols', number>>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("colsSm: { type: Number, required: false }"));
        assert!(content.contains("colsMd: { type: Number, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_record_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-record-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "type Breakpoints = 'sm' | 'md'\n",
                "export type Props<T extends string, V> =\n",
                "  Record<`${T}${Capitalize<Breakpoints>}`, V> &\n",
                "  Partial<Record<Uppercase<Breakpoints>, string>>"
            ),
        )
        .expect("write record props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props<'cols', number>>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("colsSm: { type: Number, required: true }"));
        assert!(content.contains("colsMd: { type: Number, required: true }"));
        assert!(content.contains("SM: { type: String, required: false }"));
        assert!(content.contains("MD: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_indexed_access_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-indexed-access-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Base = { name: string; count?: number; active: boolean }\n",
                "export type MethodBase = { method(): void; run: () => void; value: string }\n",
                "export type A = (string | number)[]\n",
                "export type TT = [foo: 1, bar: 'foo']\n",
                "export type ValueOf<T, K extends keyof T> = T[K]\n",
                "export type Props = {\n",
                "  label: ValueOf<Base, 'name'>\n",
                "  scalar: Base['name' | 'count']\n",
                "  active: Base['active']\n",
                "  method: MethodBase['method']\n",
                "  callable: MethodBase['run']\n",
                "  methodOrCallable: MethodBase['method'] | MethodBase['run']\n",
                "  methodOrLabel: MethodBase['method'] | MethodBase['value']\n",
                "  arrayItem: A[number]\n",
                "  tupleItem: TT[number]\n",
                "}\n",
                "export type ModelValue = A[number] | TT[number] | MethodBase['method'] | MethodBase['run']"
            ),
        )
        .expect("write indexed access props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("scalar: { type: [String, Number], required: true }"));
        assert!(content.contains("active: { type: Boolean, required: true }"));
        assert!(content.contains("method: { type: null, required: true }"));
        assert!(content.contains("callable: { type: Function, required: true }"));
        assert!(content
            .contains("methodOrCallable: { type: Function, required: true, skipCheck: true }"));
        assert!(content.contains("methodOrLabel: { type: null, required: true }"));
        assert!(content.contains("arrayItem: { type: [String, Number], required: true }"));
        assert!(content.contains("tupleItem: { type: [Number, String], required: true }"));
        assert!(content
            .contains("\"modelValue\": { type: [String, Number, Function], skipCheck: true },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_parameter_tuple_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-parameter-tuple-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Fn = (value: string, count: number, active?: boolean) => void\n",
                "export type Ctor = new (name: string, flags: boolean[]) => object\n",
                "export type Props = {\n",
                "  first: Parameters<Fn>[0]\n",
                "  anyParam: Parameters<Fn>[number]\n",
                "  ctorFirst: ConstructorParameters<Ctor>[0]\n",
                "  ctorAny: ConstructorParameters<Ctor>[number]\n",
                "  inlineParam: Parameters<(files: File[], done: () => void) => void>[number]\n",
                "}\n",
                "export type ModelValue = Parameters<Fn>[number] | ConstructorParameters<Ctor>[number]"
            ),
        )
        .expect("write parameter tuple props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("first: { type: String, required: true }"));
        assert!(content.contains("anyParam: { type: [String, Number, Boolean], required: true }"));
        assert!(content.contains("ctorFirst: { type: String, required: true }"));
        assert!(content.contains("ctorAny: { type: [String, Array], required: true }"));
        assert!(content.contains("inlineParam: { type: [Array, Function], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Boolean, Array] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_signature_parameter_tuples_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-signature-parameter-tuple-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Callable = {\n",
                "  (value: string, count: number): void\n",
                "  (active: boolean): void\n",
                "}\n",
                "export interface InterfaceCallable {\n",
                "  (name: string, flags: boolean[]): void\n",
                "}\n",
                "export type Newable = {\n",
                "  new (id: number, done: () => void): object\n",
                "}\n",
                "export interface InterfaceNewable {\n",
                "  new (label: string, enabled: boolean): object\n",
                "}\n",
                "export type Props = {\n",
                "  callAny: Parameters<Callable>[number]\n",
                "  callFirst: Parameters<InterfaceCallable>[0]\n",
                "  newAny: ConstructorParameters<Newable>[number]\n",
                "  newSecond: ConstructorParameters<InterfaceNewable>[1]\n",
                "}\n",
                "export type ModelValue = Parameters<Callable>[number] | ",
                "ConstructorParameters<InterfaceNewable>[number]"
            ),
        )
        .expect("write signature parameter tuple props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("callAny: { type: [String, Boolean, Number], required: true }"));
        assert!(content.contains("callFirst: { type: String, required: true }"));
        assert!(content.contains("newAny: { type: [Number, Function], required: true }"));
        assert!(content.contains("newSecond: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_extends_signature_parameter_tuples_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-extends-signature-parameter-tuple-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export interface Callable extends BaseCallable {\n",
                "  (active: boolean): void\n",
                "}\n",
                "export interface BaseCallable {\n",
                "  (name: string, count: number): void\n",
                "}\n",
                "export interface Newable extends BaseNewable {\n",
                "  new (label: string): object\n",
                "}\n",
                "export interface BaseNewable {\n",
                "  new (id: number, done: () => void): object\n",
                "}\n",
                "export type Props = {\n",
                "  callAny: Parameters<Callable>[number]\n",
                "  callSecond: Parameters<Callable>[1]\n",
                "  newAny: ConstructorParameters<Newable>[number]\n",
                "  newSecond: ConstructorParameters<Newable>[1]\n",
                "}\n",
                "export type ModelValue = Parameters<Callable>[number] | ",
                "ConstructorParameters<Newable>[number]"
            ),
        )
        .expect("write extends signature parameter tuple props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("callAny: { type: [Boolean, String, Number], required: true }"));
        assert!(content.contains("callSecond: { type: Number, required: true }"));
        assert!(content.contains("newAny: { type: [String, Number, Function], required: true }"));
        assert!(content.contains("newSecond: { type: Function, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String, Number, Function] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_runtime_utility_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-runtime-utility-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type MaybeText = string | null\n",
                "export type Props = {\n",
                "  label: NonNullable<MaybeText>\n",
                "  extracted: Extract<string | number | boolean, number | boolean>\n",
                "  excluded: Exclude<string | number, number>\n",
                "}\n",
                "export type ModelValue =\n",
                "  NonNullable<string | null> |\n",
                "  Extract<number | boolean, boolean> |\n",
                "  Exclude<string | number, number>"
            ),
        )
        .expect("write runtime utility props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("extracted: { type: [Number, Boolean], required: true }"));
        assert!(content.contains("excluded: { type: [String, Number], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export declare function makeLabel(): string\n",
                "export declare const makeCount: () => number\n",
                "export type BooleanFactory = () => boolean\n",
                "export type Callable = {\n",
                "  (value: string): Date\n",
                "  (value: number): Error\n",
                "}\n",
                "export interface InterfaceFactory {\n",
                "  (active: boolean): string[]\n",
                "}\n",
                "export interface ExtendedFactory extends InterfaceFactory {\n",
                "  (value: number): boolean\n",
                "}\n",
                "export type Props = {\n",
                "  label: ReturnType<typeof makeLabel>\n",
                "  count: ReturnType<typeof makeCount>\n",
                "  flag: ReturnType<BooleanFactory>\n",
                "  mixed: ReturnType<Callable>\n",
                "  list: ReturnType<InterfaceFactory>\n",
                "  extended: ReturnType<ExtendedFactory>\n",
                "}\n",
                "export type ModelValue = ",
                "ReturnType<typeof makeLabel> | ReturnType<BooleanFactory>"
            ),
        )
        .expect("write return type props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("mixed: { type: [Date, Error], required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("extended: { type: [Boolean, Array], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_default_function_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-default-function-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("named.ts"),
            concat!(
                "export default function makeDefault(): string { return '' }\n",
                "export function makeCount(): number { return 1 }"
            ),
        )
        .expect("write named default function type");
        std::fs::write(
            dir.join("anonymous.ts"),
            "export default function(): boolean { return true }",
        )
        .expect("write anonymous default function type");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import makeDefault, { makeCount } from './named'\n",
                    "import makeFlag from './anonymous'\n",
                    "type Props = {\n",
                    "  label: ReturnType<typeof makeDefault>\n",
                    "  count: ReturnType<typeof makeCount>\n",
                    "  flag: ReturnType<typeof makeFlag>\n",
                    "}\n",
                    "defineProps<Props>()\n",
                    "defineModel<",
                    "ReturnType<typeof makeDefault> | ",
                    "ReturnType<typeof makeCount> | ",
                    "ReturnType<typeof makeFlag>",
                    ">()",
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
        let expected = ["named.ts", "anonymous.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Boolean] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_function_value_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-function-value-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("factories.ts"),
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
            dir.join("arrow-default.ts"),
            "export default ((): Date => new Date())",
        )
        .expect("write default arrow function value");
        std::fs::write(
            dir.join("function-default.ts"),
            "export default (function(): Error { return new Error() })",
        )
        .expect("write default function expression value");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import makeDate from './arrow-default'\n",
                    "import makeError from './function-default'\n",
                    "import { makeLabel, makeCount, makeFlag } from './factories'\n",
                    "type Props = {\n",
                    "  label: ReturnType<typeof makeLabel>\n",
                    "  count: ReturnType<typeof makeCount>\n",
                    "  flag: ReturnType<typeof makeFlag>\n",
                    "  date: ReturnType<typeof makeDate>\n",
                    "  error: ReturnType<typeof makeError>\n",
                    "}\n",
                    "defineProps<Props>()\n",
                    "defineModel<",
                    "ReturnType<typeof makeLabel> | ",
                    "ReturnType<typeof makeCount> | ",
                    "ReturnType<typeof makeFlag> | ",
                    "ReturnType<typeof makeDate> | ",
                    "ReturnType<typeof makeError>",
                    ">()",
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
        let expected = ["factories.ts", "arrow-default.ts", "function-default.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("date: { type: Date, required: true }"));
        assert!(content.contains("error: { type: Error, required: true }"));
        assert!(
            content.contains("\"modelValue\": { type: [String, Number, Boolean, Date, Error] },")
        );
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_unannotated_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-unannotated-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("factories.ts"),
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
            dir.join("date.ts"),
            "export default function makeDate() { return new Date() }",
        )
        .expect("write default unannotated function");
        std::fs::write(
            dir.join("error.ts"),
            "export default (function() { return new Error('x') })",
        )
        .expect("write default unannotated function expression");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import makeDate from './date'\n",
                    "import makeError from './error'\n",
                    "import { makeLabel, makeCount, makeFlag, makeList, makeBox } from './factories'\n",
                    "type Props = {\n",
                    "  label: ReturnType<typeof makeLabel>\n",
                    "  count: ReturnType<typeof makeCount>\n",
                    "  flag: ReturnType<typeof makeFlag>\n",
                    "  list: ReturnType<typeof makeList>\n",
                    "  box: ReturnType<typeof makeBox>\n",
                    "  made: ReturnType<typeof import('./factories').makeFlag>\n",
                    "  created: ReturnType<typeof makeDate>\n",
                    "  error: ReturnType<typeof makeError>\n",
                    "}\n",
                    "defineProps<Props>()\n",
                    "defineModel<",
                    "ReturnType<typeof makeLabel> | ",
                    "ReturnType<typeof makeCount> | ",
                    "ReturnType<typeof makeFlag> | ",
                    "ReturnType<typeof makeList> | ",
                    "ReturnType<typeof makeBox> | ",
                    "ReturnType<typeof makeDate> | ",
                    "ReturnType<typeof makeError>",
                    ">()",
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
        let expected = ["factories.ts", "date.ts", "error.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("box: { type: Object, required: true }"));
        assert!(content.contains("made: { type: Boolean, required: true }"));
        assert!(content.contains("created: { type: Date, required: true }"));
        assert!(content.contains("error: { type: Error, required: true }"));
        assert!(content.contains(
            "\"modelValue\": { type: [String, Number, Boolean, Array, Object, Date, Error] },"
        ));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_builtin_wrapper_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-builtin-wrapper-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Props = {\n",
                "  list: ReadonlyArray<string>\n",
                "  params: Parameters<(value: string) => void>\n",
                "  map: ReadonlyMap<string, number>\n",
                "  set: ReadonlySet<string>\n",
                "  err: Error\n",
                "  maybe: MaybeRef<string[]>\n",
                "  getter: MaybeRefOrGetter<boolean>\n",
                "}\n",
                "export type ModelValue =\n",
                "  ReadonlyArray<string> |\n",
                "  ReadonlyMap<string, number> |\n",
                "  ReadonlySet<string> |\n",
                "  Error |\n",
                "  MaybeRefOrGetter<boolean> |\n",
                "  Parameters<() => void>"
            ),
        )
        .expect("write builtin wrapper props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("params: { type: Array, required: true }"));
        assert!(content.contains("map: { type: Map, required: true }"));
        assert!(content.contains("set: { type: Set, required: true }"));
        assert!(content.contains("err: { type: Error, required: true }"));
        assert!(content.contains("maybe: { type: [Object, Array], required: true }"));
        assert!(content.contains("getter: { type: [Object, Function, Boolean], required: true }"));
        assert!(content.contains(
            "\"modelValue\": { type: [Array, Map, Set, Error, Object, Function, Boolean] },"
        ));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_mapped_identity_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-mapped-identity-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type RuntimeMirror<T> = { [K in keyof T]: T[K] }\n",
                "export type Props = {\n",
                "  label: RuntimeMirror<string | number>\n",
                "  boxed: RuntimeMirror<{ value: boolean }>\n",
                "  list: RuntimeMirror<ReadonlyArray<string>>\n",
                "}\n",
                "export type ModelValue = RuntimeMirror<string | boolean>"
            ),
        )
        .expect("write mapped identity runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: [String, Number], required: true }"));
        assert!(content.contains("boxed: { type: Object, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_static_conditional_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-conditional-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Runtime<T> = ",
                "T extends 'text' ? string : ",
                "T extends 'count' ? number : boolean\n",
                "export type Props = {\n",
                "  directTrue: 'on' extends 'on' ? boolean : string\n",
                "  directFalse: 'off' extends 'on' ? boolean : string\n",
                "  text: Runtime<'text'>\n",
                "  count: Runtime<'count'>\n",
                "  active: Runtime<'active'>\n",
                "  unresolved: Runtime<'text' | 'count'>\n",
                "}\n",
                "export type ModelValue = Runtime<'text'> | Runtime<'count'> | Runtime<'active'>"
            ),
        )
        .expect("write conditional runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("directTrue: { type: Boolean, required: true }"));
        assert!(content.contains("directFalse: { type: String, required: true }"));
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("active: { type: Boolean, required: true }"));
        assert!(content.contains("unresolved: { type: null, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Boolean] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_bigint_literal_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-bigint-literal-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Big = 1n\n",
                "export type Props = {\n",
                "  literal: 1n\n",
                "  union: 1n | 'text'\n",
                "  alias: Big\n",
                "  keyword: bigint\n",
                "}\n",
                "export type ModelValue = 1n | 'text'"
            ),
        )
        .expect("write bigint literal runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("literal: { type: Number, required: true }"));
        assert!(content.contains("union: { type: [Number, String], required: true }"));
        assert!(content.contains("alias: { type: Number, required: true }"));
        assert!(content.contains("keyword: { type: null, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_type_operator_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-type-operator-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Base = { name: string; 1: boolean }\n",
                "export type Props = {\n",
                "  readonlyList: readonly string[]\n",
                "  objectKeys: keyof Base\n",
                "  literalKeys: keyof { [index: number]: string; label: string }\n",
                "  arrayKeys: keyof ReadonlyArray<string>\n",
                "  anyKeys: keyof any\n",
                "  pickedKeys: keyof Pick<Base, 'name'>\n",
                "}\n",
                "export type ModelValue = readonly boolean[] | keyof any"
            ),
        )
        .expect("write type operator runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("readonlyList: { type: Array, required: true }"));
        assert!(content.contains("objectKeys: { type: [String, Number], required: true }"));
        assert!(content.contains("literalKeys: { type: [Number, String], required: true }"));
        assert!(content.contains("arrayKeys: { type: [String, Number], required: true }"));
        assert!(content.contains("anyKeys: { type: [String, Number, Symbol], required: true }"));
        assert!(content.contains("pickedKeys: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Array, String, Number, Symbol] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_type_query_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-type-query-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export declare const text: string\n",
                "export declare const flag: boolean\n",
                "export declare const list: string[]\n",
                "export declare const boxed: { id: string }\n",
                "export type Props = {\n",
                "  text: typeof text\n",
                "  flag: typeof flag\n",
                "  list: typeof list\n",
                "  keys: keyof typeof boxed\n",
                "}\n",
                "export type ModelValue = typeof flag | typeof list"
            ),
        )
        .expect("write type query runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("keys: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, Array] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_qualified_type_query_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-qualified-type-query-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("values.ts"),
            concat!(
                "export declare const text: string\n",
                "export declare const boxed: { id: string }\n",
                "export declare const list: string[]\n"
            ),
        )
        .expect("write type query values");
        std::fs::write(dir.join("facade.ts"), "export * from './values'")
            .expect("write type query facade");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "import * as Values from './facade'\n",
                "export type Props = {\n",
                "  text: typeof Values.text\n",
                "  keys: keyof typeof Values.boxed\n",
                "  list: typeof Values.list\n",
                "}\n",
                "export type ModelValue = typeof Values.text | typeof Values.list"
            ),
        )
        .expect("write qualified type query runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
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
        let expected = ["types.ts", "facade.ts", "values.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("keys: { type: String, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Array] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_import_type_query_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-import-type-query-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("values.ts"),
            concat!(
                "export declare const text: string\n",
                "export declare const boxed: { id: string }\n",
                "export declare const list: string[]\n",
                "export declare const options: { enabled: BooleanConstructor }\n",
                "export function make(): boolean { return true }\n"
            ),
        )
        .expect("write import type query values");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Props = ExtractPropTypes<typeof import('./values').options> & {\n",
                "  text: typeof import('./values').text\n",
                "  keys: keyof typeof import('./values').boxed\n",
                "  list: typeof import('./values').list\n",
                "  made: ReturnType<typeof import('./values').make>\n",
                "}\n",
                "export type ModelValue = ",
                "typeof import('./values').text | ",
                "ReturnType<typeof import('./values').make> | ",
                "typeof import('./values').list"
            ),
        )
        .expect("write import type query runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
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
        let expected = ["types.ts", "values.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("enabled: { type: Boolean, required: false }"));
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("keys: { type: String, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("made: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean, Array] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_signature_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-signature-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Callable = { (): string }\n",
                "export type Constructable = { new (): object }\n",
                "export interface InterfaceMixed {\n",
                "  new (): object\n",
                "  value: number\n",
                "}\n",
                "export type Props = {\n",
                "  call: Callable\n",
                "  ctor: Constructable\n",
                "  ifaceMixed: InterfaceMixed\n",
                "}\n",
                "export type ModelValue = Callable | InterfaceMixed"
            ),
        )
        .expect("write signature runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("call: { type: Function, required: true }"));
        assert!(content.contains("ctor: { type: Function, required: true }"));
        assert!(content.contains("ifaceMixed: { type: [Function, Object], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Function, Object] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_intersection_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-intersection-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Callable = { (): string }\n",
                "export type Box = { value: number }\n",
                "export type Props = {\n",
                "  scalar: string & number\n",
                "  callableBox: Callable & Box\n",
                "  maybe: any | boolean\n",
                "  unknown: any\n",
                "}\n",
                "export type ModelValue = (string & number) | (Callable & Box)"
            ),
        )
        .expect("write intersection runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("scalar: { type: [String, Number], required: true }"));
        assert!(content.contains("callableBox: { type: [Function, Object], required: true }"));
        assert!(content.contains("maybe: { type: Boolean, required: true, skipCheck: true }"));
        assert!(content.contains("unknown: { type: null, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Function, Object] },"));
        assert!(!content.contains("Unknown"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }
