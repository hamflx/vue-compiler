    #[test]
    fn vue3_compile_script_infers_typescript_macro_runtime_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
interface Props { foo: string; "foo-bar"?: number }
type Emits = {(e: 'save'): void; (e: 'cancel', id: number): void}
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("interface Props"));
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"foo-bar\": { type: Number, required: false }"));
        assert!(script.content.contains(r#"emits: ["save", "cancel"],"#));
        assert!(script
            .content
            .contains("setup(__props: any, { expose: __expose, emit: __emit })"));
        assert!(script.content.contains("const props = __props"));
        assert!(script.content.contains("const emit = __emit"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("foo-bar").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("props").map(String::as_str),
            Some("setup-reactive-const")
        );
        assert_eq!(
            script.bindings.get("emit").map(String::as_str),
            Some("setup-const")
        );
    }

    #[test]
    fn vue3_compile_script_infers_with_defaults_runtime_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  count?: number
  ok?: boolean
  list?: string[]
  fn?: () => void
}>(), {
  foo: 'hi',
  count: 1,
  ok: true,
  list: () => [],
  fn() {}
})
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'hi' }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false, default: 1 }"));
        assert!(script
            .content
            .contains("ok: { type: Boolean, required: false, default: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: false, default: () => [] }"));
        assert!(script
            .content
            .contains("fn: { type: Function, required: false, default() {} }"));
        assert!(script.content.contains("const props = __props"));
        assert_eq!(
            script.bindings.get("props").map(String::as_str),
            Some("setup-const")
        );

        let prod = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(prod.errors.is_empty());
        assert!(prod.content.contains("foo: { default: 'hi' }"));
        assert!(prod.content.contains("count: { default: 1 }"));
        assert!(prod
            .content
            .contains("ok: { type: Boolean, default: true }"));
        assert!(prod.content.contains("list: { default: () => [] }"));
        assert!(prod
            .content
            .contains("fn: { type: Function, default() {} }"));
        assert!(!prod.content.contains("required:"));
    }

    #[test]
    fn vue3_compile_script_handles_ts_wrapped_define_props_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = defineProps(['foo'])! as any
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("props: ['foo'],"));
        assert!(script.content.contains("const props = __props! as any"));
        assert!(!script.content.contains("defineProps"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_infers_static_computed_with_defaults() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  quux?: () => number
  getter?: string
  asyncer?: () => Promise<number>
}>(), {
  ['foo']: 'hi',
  [`quux`]() { return 2 },
  get getter() { return 'ok' },
  async asyncer(value) { return value }
})
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(!script.content.contains("_mergeDefaults"));
        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'hi' }"));
        assert!(script
            .content
            .contains("quux: { type: Function, required: false, default() { return 2 } }"));
        assert!(script
            .content
            .contains("getter: { type: String, required: false, get default() { return 'ok' } }"));
        assert!(script.content.contains(
            "asyncer: { type: Function, required: false, async default(value) { return value } }"
        ));
    }

    #[test]
    fn vue3_compile_script_retains_prod_prop_types_for_custom_elements() {
        let mut compiler = SfcCompiler::new();
        let typed = compiler.parse(
            "Foo.ce.vue",
            r#"<script setup lang="ts">
defineProps<{ foo?: number; bar?: string; ok?: boolean }>()
</script>"#,
        );
        let script = compiler.compile_script(
            &typed,
            SfcScriptCompileOptions {
                is_prod: true,
                custom_element: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty());
        assert!(script.content.contains("foo: {type: Number}"));
        assert!(script.content.contains("bar: {type: String}"));
        assert!(script.content.contains("ok: {type: Boolean}"));

        let with_default = compiler.parse(
            "Foo.ce.vue",
            r#"<script setup lang="ts">
withDefaults(defineProps<{ foo?: number }>(), { foo: 5.5 })
</script>"#,
        );
        let script = compiler.compile_script(
            &with_default,
            SfcScriptCompileOptions {
                is_prod: true,
                custom_element: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("foo: { default: 5.5, type: Number }"));
    }

    #[test]
    fn vue3_compile_script_quotes_runtime_prop_keys_with_symbols() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
defineProps<{
  'dollar$sign': unknown
  'da-sh': unknown
}>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"dollar$sign\": { type: null, required: true }"));
        assert!(script
            .content
            .contains("\"da-sh\": { type: null, required: true }"));
    }

    #[test]
    fn vue3_compile_script_wraps_dynamic_with_defaults_runtime_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">const defaults = { foo: 'hi' }</script>
<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  ok?: boolean
  fn?: () => void
}>(), defaults)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.starts_with(
            "import { mergeDefaults as _mergeDefaults, defineComponent as _defineComponent } from 'vue'\n"
        ));
        assert!(script.content.contains("const defaults = { foo: 'hi' }"));
        assert!(script
            .content
            .contains("props: /*@__PURE__*/_mergeDefaults({"));
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("ok: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("fn: { type: Function, required: false }"));
        assert!(script.content.contains("}, defaults),"));

        let prod = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(prod.errors.is_empty());
        assert!(prod.content.contains("foo: {}"));
        assert!(prod.content.contains("ok: { type: Boolean }"));
        assert!(prod.content.contains("fn: { type: Function }"));
        assert!(prod.content.contains("}, defaults),"));
    }

    #[test]
    fn vue3_compile_script_removes_with_defaults_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { withDefaults, defineProps, ref } from 'vue'
const props = withDefaults(defineProps<{ foo?: string }>(), { foo: 'x' })
const count = ref(1)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { defineComponent as _defineComponent } from 'vue'"));
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'x' }"));
        assert!(script.content.contains("const props = __props"));
        assert!(!script.content.contains("withDefaults"));
        assert!(!script.content.contains("defineProps"));
        assert!(script.bindings.get("withDefaults").is_none());
        assert!(script.bindings.get("defineProps").is_none());
    }

    #[test]
    fn vue3_compile_script_reports_with_defaults_errors() {
        let mut compiler = SfcCompiler::new();
        let bad_first = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(foo(), { foo: 'x' })
</script>"#,
        );
        let script = compiler.compile_script(&bad_first, SfcScriptCompileOptions::default());
        assert!(
            script
                .errors
                .iter()
                .any(|error| error
                    .contains("withDefaults' first argument must be a defineProps call"))
        );
        assert!(!script.content.contains("withDefaults"));

        let runtime_props = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const props = withDefaults(defineProps({ foo: String }), { foo: 'x' })
</script>"#,
        );
        let script = compiler.compile_script(&runtime_props, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| error
            .contains("withDefaults can only be used with type-based defineProps declaration")));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(!script.content.contains("withDefaults"));
        assert!(!script.content.contains("defineProps"));

        let missing_defaults = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{ foo?: string }>())
</script>"#,
        );
        let script = compiler.compile_script(&missing_defaults, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("The 2nd argument of withDefaults is required")));
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(!script.content.contains("withDefaults"));
    }

    #[test]
    fn vue3_compile_script_warns_when_with_defaults_uses_destructure() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo } = withDefaults(defineProps<{ foo: string }>(), { foo: 'foo' })
const read = foo
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.warnings.iter().any(
            |warning| warning.contains("withDefaults() is unnecessary when using destructure")
        ));
        assert!(script.content.contains("const { foo } = __props"));
        assert!(script.content.contains("const read = foo"));
        assert!(script.props_aliases.is_empty());
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("setup-const")
        );
    }

    #[test]
    fn vue3_compile_script_reports_duplicate_define_props_and_emits() {
        let mut compiler = SfcCompiler::new();
        let duplicate_props = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
defineProps<{ foo?: string }>()
const props = withDefaults(defineProps<{ bar?: number }>(), { bar: 1 })
</script>"#,
        );
        let script = compiler.compile_script(&duplicate_props, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineProps() call")));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("withDefaults"));

        let duplicate_emits = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineEmits(['save'])
const emit = defineEmits(['cancel'])
</script>"#,
        );
        let script = compiler.compile_script(&duplicate_emits, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineEmits() call")));
        assert!(script.content.contains("const emit = __emit"));
        assert!(!script.content.contains("defineEmits"));
    }

    #[test]
    fn vue3_compile_script_reports_define_props_destructure_errors() {
        let mut compiler = SfcCompiler::new();
        let dynamic_key = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const key = 'foo'
const { [key]: foo } = defineProps(['foo'])
</script>"#,
        );
        let script = compiler.compile_script(&dynamic_key, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("destructure cannot use computed key")));

        let nested_pattern = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo: { bar } } = defineProps(['foo'])
</script>"#,
        );
        let script = compiler.compile_script(&nested_pattern, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("destructure does not support nested patterns")));

        let local_default = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let x = 1
const { foo = () => x } = defineProps(['foo'])
</script>"#,
        );
        let script = compiler.compile_script(&local_default, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot reference locally declared variables")));

        let literal_const_default = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const x = 1
const { foo = x } = defineProps(['foo'])
</script>"#,
        );
        let script =
            compiler.compile_script(&literal_const_default, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());

        let static_computed_key = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { ['foo']: foo } = defineProps(['foo'])
</script>"#,
        );
        let script =
            compiler.compile_script(&static_computed_key, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_reports_define_props_destructure_usage_errors() {
        let mut compiler = SfcCompiler::new();
        let assignment = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo } = defineProps(['foo'])
foo = 'bar'
</script>"#,
        );
        let script = compiler.compile_script(&assignment, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Cannot assign to destructured props")));

        let update = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let { foo } = defineProps(['foo'])
foo++
</script>"#,
        );
        let script = compiler.compile_script(&update, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Cannot assign to destructured props")));

        let watch_alias = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { watch as w } from 'vue'
const { foo } = defineProps(['foo'])
w(foo, () => {})
</script>"#,
        );
        let script = compiler.compile_script(&watch_alias, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch().",
            )
        }));

        let normal_script_watch_alias = compiler.parse(
            "FooBar.vue",
            r#"<script>
import { watch as w } from 'vue'
</script>
<script setup>
const { foo } = defineProps(['foo'])
w(foo, () => {})
</script>"#,
        );
        let script = compiler.compile_script(
            &normal_script_watch_alias,
            SfcScriptCompileOptions::default(),
        );

        assert!(script.errors.iter().any(|error| {
            error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch().",
            )
        }));

        let spread_argument = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { watch } from 'vue'
const { foo } = defineProps(['foo'])
watch(...[foo])
</script>"#,
        );
        let script = compiler.compile_script(&spread_argument, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let to_ref_alias = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { toRef as r } from 'vue'
const { foo } = defineProps(['foo'])
r(foo)
</script>"#,
        );
        let script = compiler.compile_script(&to_ref_alias, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to toRef().",
            )
        }));

        let shadowed = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { watch } from 'vue'
const { foo } = defineProps(['foo'])
function useLocal(foo) {
  watch(foo, () => {})
  foo++
}
const run = (foo = 1) => {
  foo++
}
</script>"#,
        );
        let script = compiler.compile_script(&shadowed, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
    }

    #[test]
    fn vue3_compile_script_reports_define_props_destructure_default_type_errors() {
        let mut compiler = SfcCompiler::new();
        let mismatch = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 'hello' } = defineProps<{ foo?: number }>()
</script>"#,
        );
        let script = compiler.compile_script(&mismatch, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains("Default value of prop \"foo\" does not match declared type.")
        }));

        let matching = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 1, bar = 'ok', enabled = true, items = [], options = {}, run = () => {} } = defineProps<{
  foo?: number
  bar?: string
  enabled?: boolean
  items?: string[]
  options?: object
  run?: () => void
}>()
</script>"#,
        );
        let script = compiler.compile_script(&matching, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let nullable = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 'hello' } = defineProps<{ foo?: number | null }>()
</script>"#,
        );
        let script = compiler.compile_script(&nullable, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let runtime_declaration = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo = 'hello' } = defineProps({ foo: Number })
</script>"#,
        );
        let script =
            compiler.compile_script(&runtime_declaration, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
    }

    #[test]
    fn vue3_compile_script_hoists_setup_only_static_literals() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "contract.vue",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\nconst msg = 'x'\nexport default /*@__PURE__*/_defineComponent({"));
        assert!(script.content.contains("const __returned__ = { msg }"));
        assert_eq!(
            script.bindings.get("msg").map(String::as_str),
            Some("literal-const")
        );
    }

    #[test]
    fn vue3_compile_script_hoist_static_uses_static_node_boundaries() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "anonymous.vue",
            r#"<script setup>
const unary = !false
const binary = 1 + 2
const regex = /.*/g
const undef = undefined
</script>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        let export_index = script.content.find("export default").unwrap();
        let setup_index = script.content.find("setup(__props)").unwrap();
        assert!(script.content.find("const unary = !false").unwrap() < export_index);
        assert!(script.content.find("const binary = 1 + 2").unwrap() < export_index);
        assert!(script.content[setup_index..].contains("const regex = /.*/g"));
        assert!(script.content[setup_index..].contains("const undef = undefined"));
        assert_eq!(
            script.bindings.get("unary").map(String::as_str),
            Some("literal-const")
        );
        assert_eq!(
            script.bindings.get("binary").map(String::as_str),
            Some("literal-const")
        );
        assert_eq!(
            script.bindings.get("regex").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("undef").map(String::as_str),
            Some("setup-maybe-ref")
        );
    }

    #[test]
    fn vue3_compile_script_hoist_static_respects_disable_and_normal_script() {
        let mut compiler = SfcCompiler::new();
        let disabled = compiler.parse(
            "anonymous.vue",
            r#"<script setup>
const foo = 'bar'
</script>"#,
        );
        let disabled_script = compiler.compile_script(
            &disabled,
            SfcScriptCompileOptions {
                hoist_static: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(
            disabled_script.errors.is_empty(),
            "{:?}",
            disabled_script.errors
        );
        assert!(!disabled_script.content.starts_with("const foo = 'bar'"));
        assert_eq!(
            disabled_script.bindings.get("foo").map(String::as_str),
            Some("setup-const")
        );

        let with_normal = compiler.parse(
            "anonymous.vue",
            r#"<script>
const normal = 'bar'
</script>
<script setup>
const foo = 'bar'
</script>"#,
        );
        let with_normal_script =
            compiler.compile_script(&with_normal, SfcScriptCompileOptions::default());

        assert!(
            with_normal_script.errors.is_empty(),
            "{:?}",
            with_normal_script.errors
        );
        assert_eq!(
            with_normal_script.bindings.get("foo").map(String::as_str),
            Some("setup-const")
        );
        let setup_index = with_normal_script.content.find("setup(").unwrap();
        assert!(with_normal_script.content[setup_index..].contains("const foo = 'bar'"));
    }

    #[test]
    fn vue3_compile_script_hoist_static_registers_runtime_enums() {
        let mut compiler = SfcCompiler::new();
        let static_enum = compiler.parse(
            "anonymous.vue",
            r#"<script setup lang="ts">
enum StaticKey { A = 1 }
</script>"#,
        );
        let static_script =
            compiler.compile_script(&static_enum, SfcScriptCompileOptions::default());

        assert!(
            static_script.errors.is_empty(),
            "{:?}",
            static_script.errors
        );
        let export_index = static_script.content.find("export default").unwrap();
        assert!(
            static_script
                .content
                .find("enum StaticKey { A = 1 }")
                .unwrap()
                < export_index
        );
        assert_eq!(
            static_script.bindings.get("StaticKey").map(String::as_str),
            Some("literal-const")
        );

        let dynamic_enum = compiler.parse(
            "anonymous.vue",
            r#"<script setup lang="ts">
let i = 0;
enum DynamicKey {
  A = 1,
  B = getCurrentInstance(),
}
const value = `template${i}`
</script>"#,
        );
        let dynamic_script = compiler.compile_script(
            &dynamic_enum,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(
            dynamic_script.errors.is_empty(),
            "{:?}",
            dynamic_script.errors
        );
        assert!(dynamic_script.content.starts_with(
            "import { defineComponent as _defineComponent } from 'vue'\n\nexport default"
        ));
        assert!(!dynamic_script.content.starts_with(
            "import { defineComponent as _defineComponent } from 'vue'\n\n\nexport default"
        ));
        let setup_index = dynamic_script.content.find("setup(__props)").unwrap();
        assert!(dynamic_script.content[setup_index..].contains("enum DynamicKey"));
        assert_eq!(
            dynamic_script
                .bindings
                .get("DynamicKey")
                .map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            dynamic_script.bindings.get("i").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            dynamic_script.bindings.get("value").map(String::as_str),
            Some("setup-const")
        );
    }

    #[test]
    fn vue3_compile_script_inlines_template_render() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { ref } from 'vue'
import ChildComp from './ChildComp.vue'
const count = ref(0)
const local = 1
const { title: heading } = defineProps(['title'])
</script>
<template><div>{{ count }} {{ local }} {{ heading }}</div><ChildComp /></template>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("toDisplayString as _toDisplayString"));
        assert!(script.content.contains("openBlock as _openBlock"));
        assert!(script
            .content
            .contains("createElementBlock as _createElementBlock"));
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script.content.contains("props: ['title'],"));
        assert!(script.content.contains("return (_ctx, _cache) => {"));
        assert!(script.content.contains("count.value"));
        assert!(script.content.contains("_toDisplayString(local)"));
        assert!(script.content.contains("_toDisplayString(__props.title)"));
        assert!(script.content.contains("_createVNode(ChildComp)"));
        assert!(!script.content.contains("const __returned__"));
        assert!(!script
            .content
            .contains("Object.defineProperty(__returned__"));
        assert_eq!(
            script.bindings.get("heading").map(String::as_str),
            Some("props-aliased")
        );
    }

    #[test]
    fn vue3_compile_script_inlines_template_props_member_component_tag() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"
  <script setup lang="ts">
    defineProps<{ Foo: { Bar: unknown } }>()
  </script>
  <template>
    <Foo.Bar/>
  </template>
  "#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxx".into()),
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        let define_component_import = script
            .content
            .find("import { defineComponent as _defineComponent } from 'vue'")
            .expect("defineComponent import");
        let render_helper_import = script
            .content
            .find("import { unref as _unref, openBlock as _openBlock, createBlock as _createBlock } from \"vue\"")
            .expect("inline render helper import");
        assert!(define_component_import < render_helper_import);
        assert!(script.content.contains(
            "import { unref as _unref, openBlock as _openBlock, createBlock as _createBlock } from \"vue\"\n\n\nexport default"
        ));
        assert!(script.content.contains("setup(__props: any)"));
        assert!(!script.content.contains("setup(__props: any, { expose"));
        assert!(script
            .content
            .contains("_createBlock(_unref(__props[\"Foo\"]).Bar)"));
        assert!(!script.content.contains("const __returned__"));
    }

    #[test]
    fn vue3_compile_script_inlines_ssr_template_render() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { ref } from 'vue'
const count = ref(0)
const style = { color: 'red' }
</script>
<template><div>{{ count }}</div><div>static</div></template>
<style>
div { color: v-bind(count) }
span { color: v-bind(style.color) }
</style>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                inline_template: true,
                inline_template_ssr: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(script.content.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(script.content.contains("__ssrInlineRender: true,"));
        assert!(script
            .content
            .contains("return (_ctx, _push, _parent, _attrs) => {"));
        assert!(script.content.contains("_ssrInterpolate(count.value)"));
        assert!(script
            .content
            .contains(r#"":--xxxxxxxx-count": (count.value)"#));
        assert!(script
            .content
            .contains(r#"":--xxxxxxxx-style\\.color": (style.color)"#));
        assert!(script.content.contains("_ssrRenderAttrs(_cssVars)"));
        assert!(!script.content.contains("_useCssVars"));
        assert!(!script.content.contains("const __returned__"));
    }

    #[test]
    fn vue3_compile_script_inlines_empty_template_render_when_missing_or_src() {
        let mut compiler = SfcCompiler::new();
        let no_template = compiler.parse("FooBar.vue", "<script setup>const a = 1</script>");
        let script = compiler.compile_script(
            &no_template,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("return () => {}"));
        assert!(!script.content.contains("const __returned__"));

        let src_template = compiler.parse(
            "FooBar.vue",
            r#"<template src="./Foo.html"></template><script setup>const a = 1</script>"#,
        );
        let script = compiler.compile_script(
            &src_template,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("return () => {}"));
        assert!(!script.content.contains("const __returned__"));
    }

    #[test]
    fn vue3_compile_script_source_map_maps_normal_script_to_vue_source() {
        let mut compiler = SfcCompiler::new();
        let source = "<script>\n  const plain = 1\n</script>";
        let descriptor = compiler.parse("FooBar.vue", source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        let map = script.map.as_ref().expect("script source map");
        assert_eq!(map.sources, vec!["FooBar.vue"]);
        assert_eq!(
            map.sources_content
                .as_ref()
                .and_then(|sources| sources.first())
                .and_then(Option::as_ref),
            Some(&source.to_string())
        );
        let original = generated_original_position(&script, "plain = 1");
        let expected = original_line_column(source, "plain = 1");
        assert_eq!(original.source, "FooBar.vue");
        assert_eq!((original.line, original.column), expected);
    }

    #[test]
    fn vue3_compile_script_source_map_can_be_disabled() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("FooBar.vue", "<script setup>const count = 1</script>");
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                source_map: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.map.is_none());
    }

    #[test]
    fn vue3_compile_script_source_map_merges_inline_template_map() {
        let mut compiler = SfcCompiler::new();
        let source = concat!(
            "<script setup>\n",
            "import { ref } from 'vue'\n",
            "const count = ref(0)\n",
            "</script>\n",
            "<template><button>{{ count }}</button></template>"
        );
        let descriptor = compiler.parse("FooBar.vue", source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("return (_ctx, _cache) => {"));
        let script_original = generated_original_position(&script, "count = ref");
        let script_expected = original_line_column(source, "count = ref");
        assert_eq!(script_original.source, "FooBar.vue");
        assert_eq!(
            (script_original.line, script_original.column),
            script_expected
        );

        let template_original = generated_original_position(&script, "\"button\"");
        let template_start = original_line_column(source, "<button>");
        let template_end = original_line_column(source, "</template>");
        assert_eq!(template_original.source, "FooBar.vue");
        assert_eq!(template_original.line, template_start.0);
        assert!(
            template_original.column >= template_start.1
                && template_original.column <= template_end.1,
            "{template_original:?}"
        );

        let expression_original = generated_original_position(&script, "count.value");
        let expression_expected = original_line_column(source, "count }}</button>");
        assert_eq!(expression_original.source, "FooBar.vue");
        assert_eq!(
            (expression_original.line, expression_original.column),
            expression_expected
        );
        assert_eq!(expression_original.name.as_deref(), Some("count"));
    }

    #[test]
    fn vue3_compile_script_source_map_maps_inline_bind_expression() {
        let mut compiler = SfcCompiler::new();
        let source = concat!(
            "<script setup>\n",
            "import { ref } from 'vue'\n",
            "const count = ref(0)\n",
            "</script>\n",
            r#"<template><button :id="count"></button></template>"#
        );
        let descriptor = compiler.parse("FooBar.vue", source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script.content.contains("id: count.value"),
            "{}",
            script.content
        );
        let expression_original = generated_original_position(&script, "count.value");
        let expression_expected = original_line_column(source, r#"count"></button>"#);
        assert_eq!(expression_original.source, "FooBar.vue");
        assert_eq!(
            (expression_original.line, expression_original.column),
            expression_expected
        );
        assert_eq!(expression_original.name.as_deref(), Some("count"));
    }
