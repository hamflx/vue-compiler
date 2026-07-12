    #[test]
    fn vue3_compile_script_resolves_external_generic_props_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types_file = dir.path().join("types.ts");
        std::fs::write(
            &types_file,
            "export type Props<T> = Readonly<Partial<T>>\nexport type Base = { ext: string }",
        )
        .expect("write generic props type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props, Base } from './types'
defineProps<Props<Base>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("ext: { type: String, required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [types_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_interface_extends_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script lang="ts">
interface Foo { x?: number }
</script>
<script setup lang="ts">
interface Bar extends Foo { y?: number }
type Extra = { extra?: boolean }
interface Props extends Bar, Extra {
  z: number
  y: string
}
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script.content.find("interface Bar extends Foo").unwrap()
                < script.content.find("interface Foo").unwrap()
        );
        assert!(script
            .content
            .contains("x: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("y: { type: String, required: true }"));
        assert!(script
            .content
            .contains("z: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("extra: { type: Boolean, required: false }"));
        assert!(!script
            .content
            .contains("y: { type: Number, required: false }"));
        assert_eq!(script.bindings.get("x").map(String::as_str), Some("props"));
        assert_eq!(script.bindings.get("y").map(String::as_str), Some("props"));
        assert_eq!(script.bindings.get("z").map(String::as_str), Some("props"));
        assert_eq!(
            script.bindings.get("extra").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_forward_interface_extends_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Props extends Base {
  own: string
}
interface Base {
  inherited?: number
}
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("own: { type: String, required: true }"));
        assert!(script
            .content
            .contains("inherited: { type: Number, required: false }"));
        assert_eq!(
            script.bindings.get("own").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("inherited").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_reports_failed_interface_extends_and_honors_vue_ignore() {
        let mut compiler = SfcCompiler::new();
        let unresolved = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
import type Base from 'unknown'
interface Props extends Base {
  local: string
}
defineProps<Props>()
</script>"#,
        );
        let unresolved_script =
            compiler.compile_script(&unresolved, SfcScriptCompileOptions::default());

        assert!(
            unresolved_script.errors.iter().any(|error| {
                error.contains("Failed to resolve extends base type")
                    && error.contains("@vue-ignore")
            }),
            "{:?}",
            unresolved_script.errors
        );
        assert!(unresolved_script
            .content
            .contains("local: { type: String, required: true }"));

        let ignored = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Base { skipped?: number }
interface Props extends /*@vue-ignore*/ Base {
  foo: string
}
defineProps<Props>()
</script>"#,
        );
        let ignored_script = compiler.compile_script(&ignored, SfcScriptCompileOptions::default());

        assert!(
            ignored_script.errors.is_empty(),
            "{:?}",
            ignored_script.errors
        );
        assert!(ignored_script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(!ignored_script.content.contains("skipped: {"));
        assert_eq!(
            ignored_script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert!(!ignored_script.bindings.contains_key("skipped"));
        assert!(ignored_script.deps.is_empty(), "{:?}", ignored_script.deps);
    }

    #[test]
    fn vue3_compile_script_honors_vue_ignore_on_property_signature_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Foo = string
defineProps<{
  foo: /* @vue-ignore */ Foo
  bar?: Foo
}>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: null, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: String, required: false }"));
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
    fn vue3_compile_script_resolves_external_interface_extends_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types_file = dir.path().join("types.ts");
        std::fs::write(
            &types_file,
            "export interface Base { ext?: string }\nexport interface Props extends Base { local: number }",
        )
        .expect("write interface props");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props } from './types'
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("ext: { type: String, required: false }"));
        assert!(script
            .content
            .contains("local: { type: Number, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [types_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_external_forward_interface_extends_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types_file = dir.path().join("types.ts");
        std::fs::write(
            &types_file,
            "export interface Props extends Base { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write interface props");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props } from './types'
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("ext: { type: String, required: false }"));
        assert!(script
            .content
            .contains("local: { type: Number, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [types_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_interface_extends_emits() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Base { (e: 'foo'): void }
interface Emits extends Base { (e: 'bar'): void }
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("emits: [\"bar\", \"foo\"],"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_forward_interface_extends_emits() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Emits extends Base { (e: 'local'): void }
interface Base { (e: 'base'): void }
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("emits: [\"local\", \"base\"],"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_forward_type_alias_intersection_emits() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Emits = Mid & {
  (e: 'local'): void
}
type Mid = Base & {
  (e: 'mid'): void
}
interface Base {
  (e: 'base'): void
}
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("emits: [\"base\", \"mid\", \"local\"],"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_define_emits_property_syntax() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Emits = {
  foo: []
  bar: [id: number]
  'foo:bar': []
}
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("emits: [\"foo\", \"bar\", \"foo:bar\"],"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_define_emits_union_function_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type BaseEmit = 'change'
type Emit = 'some' | 'emit' | BaseEmit
type Emits =
  ((e: 'foo' | 'bar') => void) |
  ((e: Emit) => void) |
  ((e: 'another', val: string) => void)
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("emits: [\"foo\", \"bar\", \"some\", \"emit\", \"change\", \"another\"],"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_reports_mixed_define_emits_type_syntax() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
const emit = defineEmits<{
  foo: []
  (e: 'bar'): void
}>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains("defineEmits() type cannot mixed call signature and property syntax.")
        }));
    }

    #[test]
    fn vue3_compile_script_resolves_external_forward_type_alias_intersection_emits_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types_file = dir.path().join("events.ts");
        std::fs::write(
            &types_file,
            "export type Emits = Mid & { (e: 'local'): void }\nexport type Mid = Base & { (e: 'mid'): void }\nexport interface Base { (e: 'base'): void }",
        )
        .expect("write type alias emits");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Emits } from './events'
const emit = defineEmits<Emits>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("emits: [\"base\", \"mid\", \"local\"],"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [types_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }
