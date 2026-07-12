    #[test]
    fn vue3_compile_script_returns_template_used_ts_import_getters() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { FooBar, FooBaz, FooQux, foo } from './x'
const fooBar: FooBar = 1
</script>
<template>
  <FooBaz></FooBaz>
  <foo-qux/>
  <foo/>
  FooBar
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains(
            "const __returned__ = { fooBar, get FooBaz() { return FooBaz }, get FooQux() { return FooQux }, get foo() { return foo } }"
        ));
        assert!(!script.content.contains("fooBar, FooBar,"));
        assert_eq!(
            script.bindings.get("FooBaz").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("FooQux").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("setup-maybe-ref")
        );
    }

    #[test]
    fn vue3_compile_script_projects_import_binding_metadata() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">
import NormalDefault from './normal.vue'
import { normalNamed } from './normal'
</script>
<script setup lang="ts">
import SetupDefault from './setup.vue'
import * as SetupNs from './ns'
import { FooBar, FooBaz, type FooType } from './x'
import { ref, defineProps } from 'vue'
const local = ref(0)
const props = defineProps<{ msg?: string }>()
const typed: FooType | null = null
</script>
<template><FooBaz />{{ local }}{{ props.msg }}</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(script.imports.len(), 8, "{:?}", script.imports);
        assert_script_import_binding(
            &script,
            "NormalDefault",
            "default",
            "./normal.vue",
            false,
            false,
            false,
        );
        assert_script_import_binding(
            &script,
            "normalNamed",
            "normalNamed",
            "./normal",
            false,
            false,
            false,
        );
        assert_script_import_binding(
            &script,
            "SetupDefault",
            "default",
            "./setup.vue",
            false,
            true,
            false,
        );
        assert_script_import_binding(&script, "SetupNs", "*", "./ns", false, true, false);
        assert_script_import_binding(&script, "FooBar", "FooBar", "./x", false, true, false);
        assert_script_import_binding(&script, "FooBaz", "FooBaz", "./x", false, true, true);
        assert_script_import_binding(&script, "FooType", "FooType", "./x", true, true, false);
        assert_script_import_binding(&script, "ref", "ref", "vue", false, true, false);
        assert!(!script.imports.contains_key("defineProps"));
    }

    #[test]
    fn vue3_compile_script_template_import_usage_handles_directives_and_dynamic_args() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { vMyDir, FooBar, foo, bar, unused, baz, msg } from './x'
</script>
<template>
  <div v-my-dir></div>
  <FooBar #[foo.slotName] />
  <FooBar #unused />
  <div :[bar.attrName]="15"></div>
  <div unused="unused"></div>
  <div #[`item:${baz.key}`]="{ value }"></div>
  <FooBar :msg />
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains(
            "const __returned__ = { get vMyDir() { return vMyDir }, get FooBar() { return FooBar }, get foo() { return foo }, get bar() { return bar }, get baz() { return baz }, get msg() { return msg } }"
        ));
        assert!(!script.content.contains("get unused()"));
    }

    #[test]
    fn vue3_compile_script_template_import_usage_ignores_ts_annotation_identifiers() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { Foo, Bar, Baz, Qux, Fred } from './x'
const a = 1
function b() {}
</script>
<template>
  {{ a as Foo }}
  {{ b<Bar>() }}
  {{ Baz }}
  <Comp v-slot="{ data }: Qux">{{ data }}</Comp>
  <div v-for="{ z = x as Qux } in list as Fred"/>
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("const __returned__ = { a, b, get Baz() { return Baz } }"));
        assert!(!script.content.contains("get Foo()"));
        assert!(!script.content.contains("get Bar()"));
        assert!(!script.content.contains("get Qux()"));
        assert!(!script.content.contains("get Fred()"));
    }

    #[test]
    fn vue3_compile_script_template_import_usage_handles_member_chains() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { Foo, Bar, Baz } from './foo'
</script>
<template>
  <div>{{ Foo.Bar.Baz }}</div>
  <div v-bind="{ ...Foo.Bar.Baz }"></div>
  <div>{{ Foo . Bar . Baz }}</div>
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("const __returned__ = { get Foo() { return Foo } }"),
            "content:\n{}\nimports:{:?}",
            script.content,
            script.imports
        );
        assert!(!script.content.contains("get Bar()"));
        assert!(!script.content.contains("get Baz()"));
        assert_eq!(
            script
                .imports
                .get("Foo")
                .map(|binding| binding.is_used_in_template),
            Some(true)
        );
        assert_eq!(
            script
                .imports
                .get("Bar")
                .map(|binding| binding.is_used_in_template),
            Some(false)
        );
        assert_eq!(
            script
                .imports
                .get("Baz")
                .map(|binding| binding.is_used_in_template),
            Some(false)
        );
    }

    #[test]
    fn vue3_compile_script_return_binding_uses_setter_for_setup_let() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let count = 0
let v = 1
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains(
            "const __returned__ = { get count() { return count }, set count(v) { count = v }, get v() { return v }, set v(_v) { v = _v } }"
        ));
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("v").map(String::as_str),
            Some("setup-let")
        );
    }

    #[test]
    fn vue3_compile_script_reports_duplicate_define_expose() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineExpose({ first: true })
defineExpose({ second: true })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineExpose() call")));
        assert!(script.content.contains("__expose({ first: true })"));
        assert!(script.content.contains("__expose({ second: true })"));
        assert!(!script.content.contains("defineExpose"));
        assert!(!script.content.contains("__expose();"));
    }

    #[test]
    fn vue3_compile_script_unbound_define_emits_only_generates_runtime_option() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineEmits(['save'])
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script
            .content
            .contains("setup(__props, { expose: __expose })"));
        assert!(!script.content.contains("emit: __emit"));
        assert!(!script.content.contains("defineEmits"));
        assert!(script.bindings.is_empty());
    }

    #[test]
    fn vue3_compile_script_removes_define_props_destructure() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz } = defineProps({ foo: String, bar: Number })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("props: { foo: String, bar: Number },"));
        assert!(script.content.contains("const __returned__ = {  }"));
        assert!(!script.content.contains("const { foo, bar: baz }"));
        assert!(!script.content.contains("defineProps"));
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
            Some("props-aliased")
        );
        assert_eq!(
            script.props_aliases.get("baz").map(String::as_str),
            Some("bar")
        );
    }

    #[test]
    fn vue3_compile_script_honors_props_destructure_option() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz, nested: { deep }, ...rest } = defineProps({ foo: String, bar: Number, nested: Object })
const message = foo + baz + deep + rest.extra
</script>"#,
        );
        let disabled = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                props_destructure: SfcPropsDestructureMode::Disabled,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(disabled.errors.is_empty(), "{:?}", disabled.errors);
        assert!(disabled
            .content
            .contains("const { foo, bar: baz, nested: { deep }, ...rest } = __props"));
        assert!(disabled
            .content
            .contains("const message = foo + baz + deep + rest.extra"));
        assert!(!disabled.content.contains("_createPropsRestProxy"));
        assert!(!disabled.content.contains("__props.foo + __props.bar"));
        assert_eq!(
            disabled.bindings.get("foo").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            disabled.bindings.get("baz").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            disabled.bindings.get("deep").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            disabled.bindings.get("rest").map(String::as_str),
            Some("setup-const")
        );

        let errored = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                props_destructure: SfcPropsDestructureMode::Error,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(errored
            .errors
            .iter()
            .any(|error| error.contains("Props destructure is explicitly prohibited via config.")));
    }

    #[test]
    fn vue3_compile_script_rewrites_define_props_destructure_references() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz, 'foo.bar': fooBar } = defineProps({ foo: String, bar: Number, 'foo.bar': Boolean })
const message = foo + baz
const payload = { foo, baz, fooBar }
function read(foo) {
  return foo + baz
}
for (const baz of [1]) {
  console.log(baz, foo)
}
console.log(message, payload, fooBar)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(!script
            .content
            .contains("const { foo, bar: baz, 'foo.bar': fooBar }"));
        assert!(script
            .content
            .contains("const message = __props.foo + __props.bar"));
        assert!(script.content.contains(
            r#"const payload = { foo: __props.foo, baz: __props.bar, fooBar: __props["foo.bar"] }"#
        ));
        assert!(script
            .content
            .contains("function read(foo) {\n  return foo + __props.bar\n}"));
        assert!(script.content.contains("console.log(baz, __props.foo)"));
        assert!(script
            .content
            .contains(r#"console.log(message, payload, __props["foo.bar"])"#));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props-aliased")
        );
        assert_eq!(
            script.bindings.get("fooBar").map(String::as_str),
            Some("props-aliased")
        );
        assert_eq!(
            script.props_aliases.get("baz").map(String::as_str),
            Some("bar")
        );
        assert_eq!(
            script.props_aliases.get("fooBar").map(String::as_str),
            Some("foo.bar")
        );
        assert!(!script.props_aliases.contains_key("foo"));
    }

    #[test]
    fn vue3_compile_script_generates_define_props_destructure_rest_proxy() {
        let mut compiler = SfcCompiler::new();
        let runtime = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz, ...rest } = defineProps(['foo', 'bar', 'baz'])
const read = foo + baz + rest.baz
</script>"#,
        );
        let script = compiler.compile_script(&runtime, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { createPropsRestProxy as _createPropsRestProxy } from 'vue'\n"));
        assert!(script
            .content
            .contains(r#"const rest = _createPropsRestProxy(__props, ["foo","bar"])"#));
        assert!(script
            .content
            .contains("const read = __props.foo + __props.bar + rest.baz"));
        assert!(!script.content.contains("const { foo, bar: baz, ...rest }"));
        assert!(!script.content.contains("defineProps"));
        assert!(script
            .content
            .contains("const __returned__ = { rest, read }"));
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
            Some("props-aliased")
        );
        assert_eq!(
            script.bindings.get("rest").map(String::as_str),
            Some("setup-reactive-const")
        );

        let typed = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo, ...rest } = defineProps<{ foo?: string, bar?: number }>()
</script>"#,
        );
        let script = compiler.compile_script(&typed, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.starts_with(
            "import { createPropsRestProxy as _createPropsRestProxy, defineComponent as _defineComponent } from 'vue'\n"
        ));
        assert!(script.content.contains("setup(__props: any"));
        assert!(script
            .content
            .contains(r#"const rest = _createPropsRestProxy(__props, ["foo"])"#));
    }

    #[test]
    fn vue3_compile_script_merges_define_props_destructure_defaults() {
        let mut compiler = SfcCompiler::new();
        let runtime = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const external = 'x'
const { foo = 1, bar = {}, func = () => {}, ext = external, 'foo:bar': fooBar = 'foo-bar' } = defineProps(['foo', 'bar', 'func', 'ext', 'foo:bar'])
</script>"#,
        );
        let script = compiler.compile_script(&runtime, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { mergeDefaults as _mergeDefaults } from 'vue'\n"));
        assert!(script.content.contains(
            "props: /*@__PURE__*/_mergeDefaults(['foo', 'bar', 'func', 'ext', 'foo:bar'], {"
        ));
        assert!(script.content.contains("foo: 1"));
        assert!(script.content.contains("bar: () => ({})"));
        assert!(script.content.contains("func: () => {}, __skip_func: true"));
        assert!(script.content.contains("ext: external, __skip_ext: true"));
        assert!(script.content.contains(r#""foo:bar": 'foo-bar'"#));
        assert!(!script.content.contains("const { foo = 1"));

        let typed = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 1, bar = {}, func = () => {}, label = 'x' } = defineProps<{
  foo?: number
  bar?: object
  func?: () => void
  label?: string
}>()
</script>"#,
        );
        let script = compiler.compile_script(&typed, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("foo: { type: Number, required: false, default: 1 }"));
        assert!(script
            .content
            .contains("bar: { type: Object, required: false, default: () => ({}) }"));
        assert!(script
            .content
            .contains("func: { type: Function, required: false, default: () => {} }"));
        assert!(script
            .content
            .contains("label: { type: String, required: false, default: 'x' }"));

        let prod = compiler.compile_script(
            &typed,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(prod.errors.is_empty(), "{:?}", prod.errors);
        assert!(prod.content.contains("foo: { default: 1 }"));
        assert!(prod.content.contains("bar: { default: () => ({}) }"));
        assert!(prod
            .content
            .contains("func: { type: Function, default: () => {} }"));
        assert!(prod.content.contains("label: { default: 'x' }"));
    }

    #[test]
    fn vue3_compile_script_merges_default_with_runtime_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script>export default { name: 'X' }</script>
<script setup>
const props = defineProps({ foo: String })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("__name: 'FooBar'"));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("const props = __props"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_wraps_typescript_runtime_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = defineProps({ foo: String })
const emit = defineEmits(['save'])
defineExpose({ reset() {} })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/_defineComponent({"));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("const props = __props"));
        assert!(script.content.contains("const emit = __emit"));
    }

    #[test]
    fn vue3_compile_script_merges_define_options_runtime() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { defineOptions, ref } from 'vue'
defineOptions({ name: 'FooApp', inheritAttrs: false })
const a = ref(1)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script.content.contains(
            "export default /*@__PURE__*/Object.assign({ name: 'FooApp', inheritAttrs: false }, {"
        ));
        assert!(script.content.contains("__name: 'FooBar',"));
        assert!(script.content.contains("const __returned__ = { a, ref }"));
        assert!(!script.content.contains("defineOptions"));

        let empty = compiler.parse("FooBar.vue", "<script setup>defineOptions()</script>");
        let empty_script = compiler.compile_script(&empty, SfcScriptCompileOptions::default());
        assert!(empty_script.errors.is_empty());
        assert!(empty_script.content.contains("export default {"));
        assert!(!empty_script.content.contains("Object.assign"));
        assert!(!empty_script.content.contains("defineOptions"));
    }

    #[test]
    fn vue3_compile_script_spreads_define_options_in_typescript_wrapper() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">export default { custom: true }</script>
<script setup lang="ts">
defineOptions({ name: 'FooApp' } as any)
const a: number = 1
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("const __default__ = { custom: true }"));
        assert!(script.content.contains(
            "export default /*@__PURE__*/_defineComponent({\n  ...__default__,\n  ...{ name: 'FooApp' },"
        ));
        assert!(script
            .content
            .contains("const a: number = 1\n\nconst __returned__ = { a }"));
        assert!(!script.content.contains("defineOptions"));
    }

    #[test]
    fn vue3_compile_script_reports_define_options_errors() {
        let mut compiler = SfcCompiler::new();
        let duplicate = compiler.parse(
            "FooBar.vue",
            "<script setup>defineOptions({}); defineOptions({})</script>",
        );
        let script = compiler.compile_script(&duplicate, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineOptions() call")));

        let invalid_option = compiler.parse(
            "FooBar.vue",
            "<script setup>defineOptions({ props: [] })</script>",
        );
        let script = compiler.compile_script(&invalid_option, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot be used to declare props")));

        let string_key = compiler.parse(
            "FooBar.vue",
            "<script setup>defineOptions({ 'props': [] })</script>",
        );
        let script = compiler.compile_script(&string_key, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());

        let type_argument = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">defineOptions<{ name: 'FooApp' }>()</script>"#,
        );
        let script = compiler.compile_script(&type_argument, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot accept type arguments")));

        let assigned = compiler.parse(
            "FooBar.vue",
            "<script setup>const options = defineOptions({ name: 'FooApp' })</script>",
        );
        let script = compiler.compile_script(&assigned, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("has no returning value")));

        let aliased = compiler.parse(
            "FooBar.vue",
            "<script setup>import { defineOptions as d } from 'vue'\nd({ name: 'FooApp' })</script>",
        );
        let script = compiler.compile_script(&aliased, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot be aliased to a different name")));
        assert!(!script.content.contains("defineOptions as d"));
    }

    #[test]
    fn vue3_compile_script_generates_define_model_runtime() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { defineModel, ref } from 'vue'
const modelValue = defineModel({ required: true })
const c = defineModel('count')
const title = defineModel(`title`, { default: 'x' })
const other = ref(1)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { useModel as _useModel } from 'vue'"));
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script
            .content
            .contains("\"modelValue\": { required: true },"));
        assert!(script.content.contains("\"modelModifiers\": {},"));
        assert!(script.content.contains("\"count\": {},"));
        assert!(script.content.contains("\"countModifiers\": {},"));
        assert!(script.content.contains("\"title\": { default: 'x' },"));
        assert!(script.content.contains("\"titleModifiers\": {},"));
        assert!(script
            .content
            .contains("emits: [\"update:modelValue\", \"update:count\", \"update:title\"],"));
        assert!(script
            .content
            .contains(r#"const modelValue = _useModel(__props, "modelValue")"#));
        assert!(script
            .content
            .contains("const c = _useModel(__props, 'count')"));
        assert!(script
            .content
            .contains("const title = _useModel(__props, `title`)"));
        assert!(script
            .content
            .contains("const __returned__ = { modelValue, c, title, other, ref }"));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("c").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("title").map(String::as_str),
            Some("setup-ref")
        );
        assert!(!script.bindings.contains_key("defineModel"));
    }

    #[test]
    fn vue3_compile_script_merges_define_model_with_props_and_emits() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineProps({ foo: String })
defineEmits(['change'])
const count = defineModel({ default: 0 })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { useModel as _useModel, mergeModels as _mergeModels } from 'vue'"));
        assert!(script
            .content
            .contains("props: /*@__PURE__*/_mergeModels({ foo: String }, {"));
        assert!(script.content.contains("\"modelValue\": { default: 0 },"));
        assert!(script.content.contains("\"modelModifiers\": {},"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels(['change'], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains(r#"const count = _useModel(__props, "modelValue")"#));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-ref")
        );
    }

    #[test]
    fn vue3_resolve_type_projects_props_calls_deps_and_errors() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("props.ts"),
            "export type Props = { foo: number; bar?: string; (e: 'save'): void }",
        )
        .expect("write props");
        let filename = dir.path().join("Comp.vue");
        let mut compiler = SfcCompiler::new();
        let resolved = compiler.resolve_vue3_type(
            filename.to_string_lossy(),
            "import type { Props } from './props'\ndefineProps<Props>()",
            SfcScriptCompileOptions::default(),
        );

        assert!(resolved.errors.is_empty(), "{:?}", resolved.errors);
        assert_eq!(resolved.props.get("foo"), Some(&vec!["Number".to_string()]));
        assert_eq!(resolved.props.get("bar"), Some(&vec!["String".to_string()]));
        assert_eq!(
            resolved.raw.props.get("bar").map(|prop| prop.optional),
            Some(true)
        );
        assert_eq!(resolved.calls.len(), 1);
        assert_eq!(
            resolved.deps,
            vec![dir
                .path()
                .join("props.ts")
                .to_string_lossy()
                .replace('\\', "/")]
        );

        let failed = compiler.resolve_vue3_type(
            "Broken.vue",
            "defineProps<Missing>()",
            SfcScriptCompileOptions::default(),
        );
        assert!(failed
            .errors
            .iter()
            .any(|error| error.contains("Unresolvable type reference")));
    }

    #[test]
    fn vue3_compile_script_reports_duplicate_define_model_names() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const a = defineModel('count')
const b = defineModel('count')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate model name \"count\"")));
    }

    #[test]
    fn vue3_compile_script_rewrites_unbound_define_model_expression() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineModel('count')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("\"count\": {},"));
        assert!(script.content.contains("\"countModifiers\": {},"));
        assert!(script.content.contains(r#"emits: ["update:count"],"#));
        assert!(
            script.content.contains(" _useModel(__props, 'count')")
                || script.content.contains("\n_useModel(__props, 'count')")
        );
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_splits_define_model_get_set_transformers() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel({
  get(v) { return v - 1 },
  set: (v) => { return v + 1 },
  required: true
})
const count = defineModel('count', {
  default: 0,
  get(v) { return v - 1 },
  required: true,
  set: (v) => { return v + 1 },
})
const value = defineModel<number>('value', {
  get(v) { return v },
  required: true,
})
const only = defineModel('only', {
  "get": (v) => v - 1,
  "set": (v) => v + 1,
})
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        let compact = compact_js_whitespace(&script.content);
        assert!(compact.contains("\"modelValue\": { required: true },"));
        assert!(compact.contains("\"count\": { default: 0, required: true, },"));
        assert!(compact.contains("\"value\": { type: Number, ...{ required: true, } },"));
        assert!(compact.contains("\"only\": { },"));
        assert!(compact.contains("const modelValue = _useModel(__props, \"modelValue\", { get(v) { return v - 1 }, set: (v) => { return v + 1 }, })"));
        assert!(compact.contains("const count = _useModel(__props, 'count', { get(v) { return v - 1 }, set: (v) => { return v + 1 }, })"));
        assert!(compact.contains(
            "const value = _useModel<number>(__props, 'value', { get(v) { return v }, })"
        ));
        assert!(compact.contains("const only = _useModel(__props, 'only', { \"get\": (v) => v - 1, \"set\": (v) => v + 1, })"));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("value").map(String::as_str),
            Some("setup-ref")
        );
    }

    #[test]
    fn vue3_compile_script_keeps_dynamic_define_model_options_unsplit() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const extra = { required: true }
const key = 'required'
const spread = defineModel({ get(v) { return v }, ...extra })
const computed = defineModel('computed', { get(v) { return v }, [key]: true })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"modelValue\": { get(v) { return v }, ...extra },"));
        assert!(script
            .content
            .contains("\"computed\": { get(v) { return v }, [key]: true },"));
        assert!(script.content.contains(
            "const spread = _useModel(__props, \"modelValue\", { get(v) { return v }, ...extra })"
        ));
        assert!(script.content.contains(
            "const computed = _useModel(__props, 'computed', { get(v) { return v }, [key]: true })"
        ));
        assert!(!script.content.contains("defineModel"));
    }

    #[test]
    fn vue3_compile_script_reports_macro_runtime_scope_references() {
        let mut compiler = SfcCompiler::new();

        let props = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let local = 1
defineProps({ foo: { default: () => local } })
</script>"#,
        );
        let script = compiler.compile_script(&props, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("`defineProps()`") && error.contains("cannot reference locally declared")
        }));

        let emits = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineEmits([eventName])
let eventName = 'save'
</script>"#,
        );
        let script = compiler.compile_script(&emits, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("`defineEmits()`") && error.contains("cannot reference locally declared")
        }));

        let model = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const options = { required: true }
const value = defineModel({ default: () => options })
</script>"#,
        );
        let script = compiler.compile_script(&model, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("`defineModel()`") && error.contains("cannot reference locally declared")
        }));

        let allowed = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const propKey = 'foo'
const eventName = 'save'
let value = 1
let dynamicKey = 'required'
const extra = { required: true }
defineProps([propKey])
defineEmits([eventName])
const modelValue = defineModel({
  default: eventName,
  get(v) { return value },
  set(v) { value = v },
  [dynamicKey]: true,
  ...extra
})
</script>"#,
        );
        let script = compiler.compile_script(&allowed, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("props: /*@__PURE__*/_mergeModels([propKey], {"));
        assert!(script.content.contains("\"modelValue\": {"));
        assert!(script.content.contains("default: eventName"));
        assert!(script.content.contains("[dynamicKey]: true"));
        assert!(script.content.contains("...extra"));
        assert!(!script.content.contains("defineModel"));
    }

    #[test]
    fn vue3_compile_script_infers_define_model_typescript_runtime_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel<boolean | string>()
const count = defineModel<number>('count')
const disabled = defineModel<number>('disabled', { required: false })
const any = defineModel<any | boolean>('any')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains(
            "import { useModel as _useModel, defineComponent as _defineComponent } from 'vue'"
        ));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(script.content.contains("\"modelModifiers\": {},"));
        assert!(script.content.contains("\"count\": { type: Number },"));
        assert!(script
            .content
            .contains("\"disabled\": { type: Number, ...{ required: false } },"));
        assert!(script
            .content
            .contains("\"any\": { type: Boolean, skipCheck: true },"));
        assert!(script.content.contains(
            "emits: [\"update:modelValue\", \"update:count\", \"update:disabled\", \"update:any\"],"
        ));
        assert!(script
            .content
            .contains(r#"const modelValue = _useModel<boolean | string>(__props, "modelValue")"#));
        assert!(script
            .content
            .contains("const count = _useModel<number>(__props, 'count')"));
        assert!(script
            .content
            .contains("const disabled = _useModel<number>(__props, 'disabled')"));
        assert!(script
            .content
            .contains("const any = _useModel<any | boolean>(__props, 'any')"));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("disabled").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("any").map(String::as_str),
            Some("setup-ref")
        );
    }

    #[test]
    fn vue3_compile_script_erases_define_model_types_in_production() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel<boolean>()
const fn = defineModel<() => void>('fn')
const fnWithDefault = defineModel<() => void>('fnWithDefault', { default: () => null })
const str = defineModel<string>('str')
const optional = defineModel<string>('optional', { required: false })
</script>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"modelValue\": { type: Boolean },"));
        assert!(script.content.contains("\"fn\": {},"));
        assert!(script
            .content
            .contains("\"fnWithDefault\": { type: Function, ...{ default: () => null } },"));
        assert!(script.content.contains("\"str\": {},"));
        assert!(script
            .content
            .contains("\"optional\": { required: false },"));
        assert!(script.content.contains(
            "emits: [\"update:modelValue\", \"update:fn\", \"update:fnWithDefault\", \"update:str\", \"update:optional\"],"
        ));
        assert!(script
            .content
            .contains(r#"const modelValue = _useModel<boolean>(__props, "modelValue")"#));
        assert!(script
            .content
            .contains("const fn = _useModel<() => void>(__props, 'fn')"));
        assert!(script
            .content
            .contains("const str = _useModel<string>(__props, 'str')"));

        let mixed = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel<boolean | string | {}>()
const value = defineModel<number | (() => number)>('value', { default: () => 1 })
</script>"#,
        );
        let mixed_script = compiler.compile_script(
            &mixed,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(mixed_script.errors.is_empty());
        assert!(mixed_script
            .content
            .contains("\"modelValue\": { type: [Boolean, String, Object] },"));
        assert!(mixed_script
            .content
            .contains("\"value\": { type: [Number, Function], ...{ default: () => 1 } },"));
    }

    #[test]
    fn vue3_compile_script_resolves_define_model_type_aliases() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">
type NormalMaybe = any | boolean
</script>
<script setup lang="ts">
type SetupMaybe = any | boolean
const setupAlias = defineModel<SetupMaybe>('setupAlias')
const normalAlias = defineModel<NormalMaybe>('normalAlias')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"setupAlias\": { type: Boolean, skipCheck: true },"));
        assert!(script
            .content
            .contains("\"normalAlias\": { type: Boolean, skipCheck: true },"));
        assert!(script
            .content
            .contains("const setupAlias = _useModel<SetupMaybe>(__props, 'setupAlias')"));
        assert!(script
            .content
            .contains("const normalAlias = _useModel<NormalMaybe>(__props, 'normalAlias')"));
    }
