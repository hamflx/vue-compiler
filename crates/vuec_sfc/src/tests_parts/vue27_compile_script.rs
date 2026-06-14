    #[test]
    fn vue27_prefix_identifiers_rewrites_render_scope_references() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _c('div',{style:{color}},[_v(_s(foo)),_l(list,function(i){return _c('p',[_v(_s(i))])})])}}";

        assert_eq!(
            compiler.prefix_vue27_identifiers(
                source,
                Vue27PrefixIdentifiersOptions::default()
            ),
            "function render(){var _vm=this,_c=_vm._self._c;return _c('div',{style:{color: _vm.color}},[_vm._v(_vm._s(_vm.foo)),_vm._l(_vm.list,function(i){return _c('p',[_vm._v(_vm._s(i))])})])}"
        );
    }
    #[test]
    fn vue27_prefix_identifiers_uses_setup_proxy_for_setup_bindings() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _c('div',{on:{click:function($event){count++}}},[_v(_s(count))])}}";
        let options = Vue27PrefixIdentifiersOptions {
            bindings: BTreeMap::from([("count".into(), "setup-ref".into())]),
            ..Vue27PrefixIdentifiersOptions::default()
        };

        assert_eq!(
            compiler.prefix_vue27_identifiers(source, options),
            "function render(){var _vm=this,_c=_vm._self._c,_setup=_vm._self._setupProxy;return _c('div',{on:{click:function($event){_setup.count++}}},[_vm._v(_vm._s(_setup.count))])}"
        );
    }

    #[test]
    fn vue27_prefix_identifiers_rewrites_template_literal_references() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _c('div',{attrs:{class:`lvl${level}`,\"aria-label\":`Last Page, Page ${state.nbPages}`}},[_v(_s(label))])}}";

        assert_eq!(
            compiler.prefix_vue27_identifiers(
                source,
                Vue27PrefixIdentifiersOptions::default()
            ),
            "function render(){var _vm=this,_c=_vm._self._c;return _c('div',{attrs:{class:`lvl${_vm.level}`,\"aria-label\":`Last Page, Page ${_vm.state.nbPages}`}},[_vm._v(_vm._s(_vm.label))])}"
        );
    }

    #[test]
    fn vue27_prefix_identifiers_keeps_template_literal_locals() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _l(items,function(item){return _c('div',{attrs:{title:`item ${item.label} of ${total}`}})})}}";

        assert_eq!(
            compiler.prefix_vue27_identifiers(
                source,
                Vue27PrefixIdentifiersOptions::default()
            ),
            "function render(){var _vm=this,_c=_vm._self._c;return _vm._l(_vm.items,function(item){return _c('div',{attrs:{title:`item ${item.label} of ${_vm.total}`}})})}"
        );
    }

    #[test]
    fn vue27_prefix_identifiers_rewrites_template_literal_setup_bindings() {
        let compiler = SfcCompiler::new();
        let source =
            "function render(){with(this){return _c('div',{attrs:{title:`Count ${count}`}})}}";
        let options = Vue27PrefixIdentifiersOptions {
            bindings: BTreeMap::from([("count".into(), "setup-ref".into())]),
            ..Vue27PrefixIdentifiersOptions::default()
        };

        assert_eq!(
            compiler.prefix_vue27_identifiers(source, options),
            "function render(){var _vm=this,_c=_vm._self._c,_setup=_vm._self._setupProxy;return _c('div',{attrs:{title:`Count ${_setup.count}`}})}"
        );
    }

    #[test]
    fn vue27_prefix_identifiers_rewrites_object_spread_references() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _l(items,function(item){return getNode(itemSlots.default,{...slotProps, option:item, labelKey, valueKey})})}}";

        assert_eq!(
            compiler.prefix_vue27_identifiers(
                source,
                Vue27PrefixIdentifiersOptions::default()
            ),
            "function render(){var _vm=this,_c=_vm._self._c;return _vm._l(_vm.items,function(item){return _vm.getNode(_vm.itemSlots.default,{..._vm.slotProps, option:item, labelKey: _vm.labelKey, valueKey: _vm.valueKey})})}"
        );
    }

    #[test]
    fn vue27_prefix_identifiers_rewrites_new_expression_arguments() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _c('span',[_v(_s(new Date(value).toLocaleString())),_v(_s(new Formatter(locale)))])}}";

        assert_eq!(
            compiler.prefix_vue27_identifiers(
                source,
                Vue27PrefixIdentifiersOptions::default()
            ),
            "function render(){var _vm=this,_c=_vm._self._c;return _c('span',[_vm._v(_vm._s(new Date(_vm.value).toLocaleString())),_vm._v(_vm._s(new _vm.Formatter(_vm.locale)))])}"
        );
    }

    #[test]
    fn vue27_sfc_template_code_uses_official_identifier_prefixing() {
        let compiler = SfcCompiler::new();
        let render = "with(this){return _c('el-breadcrumb',_l((levelList),function(item,index){return _c('el-breadcrumb-item',{key:item.path},[_c('a',{on:{\"click\":function($event){$event.preventDefault();return handleLink(item)}}},[_v(_s(item.meta.title))])])}),1)}";

        let code = compiler.vue27_sfc_template_code(
            render,
            &[],
            Vue27PrefixIdentifiersOptions::default(),
            false,
        );

        assert!(code.contains("_vm._l((_vm.levelList),function(item,index)"));
        assert!(code.contains("return _vm.handleLink(item)"));
        assert!(code.contains("_vm._s(item.meta.title)"));
        assert!(!code.contains("_vm.item.meta.title"));
        assert!(code.contains("render._withStripped = true"));
    }

    #[test]
    fn vue27_sfc_template_code_prefixes_identifier_named_render() {
        let compiler = SfcCompiler::new();
        let code = compiler.vue27_sfc_template_code(
            "with(this){return _c('div',[_c('p',[_v(_s(render))])])}",
            &[],
            Vue27PrefixIdentifiersOptions::default(),
            false,
        );

        assert!(code.contains("var render = function render()"));
        assert!(code.contains("_vm._s(_vm.render)"), "{code}");
        assert!(!code.contains("_vm._s(render)"), "{code}");
    }

    #[test]
    fn vue27_sfc_template_code_prefixes_static_render_functions() {
        let compiler = SfcCompiler::new();
        let code = compiler.vue27_sfc_template_code(
            "with(this){return _m(0)}",
            &["with(this){return _c('div',[_v(_s(msg))])}".to_string()],
            Vue27PrefixIdentifiersOptions::default(),
            false,
        );

        assert!(code.contains("return _vm._m(0)"));
        assert!(code.contains(
            "function (){var _vm=this,_c=_vm._self._c;return _c('div',[_vm._v(_vm._s(_vm.msg))])"
        ));
    }

    #[test]
    fn vue27_compile_script_injects_normal_script_css_vars() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            "<script>const a = 1</script><style>div{ color: v-bind(color); }</style>",
        );
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("const __default__ = {}"));
        assert!(script
            .content
            .contains("import { useCssVars as _useCssVars } from 'vue'"));
        assert!(script.content.contains("\"xxxxxxxx-color\": (_vm.color)"));
        assert!(script.content.contains("export default __default__"));
    }

    #[test]
    fn vue27_compile_script_uses_legacy_css_var_names_and_comment_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            "<script>const a = 1</script><style>// color: v-bind(color)\ndiv{ font-size: v-bind('font.size'); }</style>",
        );
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("\"xxxxxxxx-color\": (_vm.color)"));
        assert!(script
            .content
            .contains("\"xxxxxxxx-font_size\": (_vm.font.size)"));
    }

    #[test]
    fn vue27_compile_script_injects_setup_css_vars_with_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
import { defineProps, ref } from 'vue'
const color = 'red'
const size = ref('10px')
defineProps({ foo: String })
</script><style>div{ color: v-bind(color); width: v-bind(size); border: v-bind(foo); }</style>"#,
        );
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("props: { foo: String },"));
        assert!(script
            .content
            .contains("\"xxxxxxxx-color\": (_setup.color)"));
        assert!(script.content.contains("\"xxxxxxxx-size\": (_setup.size)"));
        assert!(script.content.contains("\"xxxxxxxx-foo\": (_vm.foo)"));
        assert!(script
            .content
            .contains("return { __sfc: true,color, size, ref }"));
        assert!(!script.content.contains("defineProps"));
    }

    #[test]
    fn vue27_compile_script_can_omit_script_setup_marker_for_official_tests() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("foo.vue", "<script setup>const color = 'red'</script>");
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("return { color }"));
        assert!(!script.content.contains("__sfc: true"));
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn vue27_compile_script_preserves_official_empty_test_return_spacing() {
        let mut compiler = SfcCompiler::new();
        let descriptor =
            compiler.parse("foo.vue", "<script setup>defineExpose({ foo: 1 })</script>");
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("return {  }"));
        assert!(!script.content.contains("return {}"));
    }

    #[test]
    fn vue27_compile_script_reports_options_and_inject_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
export default {
  inject: ['foo', 'bar'],
  props: { baz: String },
  setup() { return { qux: null } },
  data() { return { quux: null } },
  methods: { quuz() {} },
  computed: { corge() {} }
}
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("bar").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("qux").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("quux").map(String::as_str),
            Some("data")
        );
        assert_eq!(
            script.bindings.get("quuz").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("corge").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("false")
        );
    }

    #[test]
    fn vue27_compile_script_merges_normal_script_bindings_into_setup_metadata() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
import { xx } from './x'
export const aa = 1
let bb = 2
function cc() {}
class dd {}
</script>
<script setup>
import { ref as r } from 'vue'
import { x } from './x'
const a = r(1)
let b = 2
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(
            script.bindings.get("xx").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("aa").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("bb").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("cc").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("dd").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("x").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("r").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("a").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("b").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn vue27_compile_script_orders_normal_and_setup_module_chunks_like_vue27() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
export const n = 1
export default{
  some:'option'
}
</script>
<script setup>
import { x } from './x'
x()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(
            script.content.find("import { x } from './x'").unwrap()
                < script.content.find("export const n = 1").unwrap()
        );
        assert!(script
            .content
            .contains("export const n = 1\nconst __default__ = {\n  some:'option'"));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
import { x } from './x'
x()
</script>
<script>
export const n = 1
const def = {}
export { def as default }
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(
            script.content.find("export const n = 1").unwrap()
                < script.content.find("import { x } from './x'").unwrap()
        );
        assert!(script.content.contains("const __default__ = def"));
    }

    #[test]
    fn vue27_compile_script_hoists_side_effect_imports_and_dedupes_setup_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
import { x } from './x'
</script>
<script setup>
import { x } from './x'
import { ref } from 'vue'
import 'foo/css'
x()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(script.content.matches("import { x } from './x'").count(), 1);
        assert!(script
            .content
            .contains("import { ref } from 'vue'\nimport 'foo/css'"));
        assert!(script.content.contains("return { __sfc: true,x, ref }"));
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_reports_script_setup_macro_errors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>foo()</script><script setup lang="ts">bar()</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors[0].contains("same language type"));

        let descriptor = compiler.parse("foo.vue", "<script setup>export const a = 1</script>");
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors[0].contains("cannot contain ES module exports"));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">defineProps<{}>({})</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors[0].contains("cannot accept both type and non-type arguments"));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const bar = 1
defineProps({ foo: { default: () => bar } })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot reference locally declared variables")));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>const bar = 1</script>
<script setup>
defineProps({ foo: { default: () => bar } })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_returns_top_level_normal_and_setup_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
import { x } from './x'
let a = 1
const b = 2
function c() {}
class d {}
</script>
<script>
import { xx } from './x'
let aa = 1
const bb = 2
function cc() {}
class dd {}
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("return { __sfc: true,aa, bb, cc, dd, a, b, c, d, xx, x }"));
    }

    #[test]
    fn vue27_compile_script_filters_ts_template_import_usage() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
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
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("return { __sfc: true,fooBar, FooBaz, FooQux, foo }"));
        assert!(!script.content.contains("return { fooBar, FooBar,"));
    }

    #[test]
    fn vue27_compile_script_filters_template_string_import_usage() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
import { VAR, VAR2, VAR3 } from './x'
</script>
<template>
  {{ `${VAR}VAR2${VAR3}` }}
</template>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("return { __sfc: true,VAR, VAR3 }"));
    }

    #[test]
    fn vue27_compile_script_filters_import_type_return_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
import type { Foo } from './main.ts'
import { type Bar, Baz } from './main.ts'
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("return { __sfc: true,Baz }"));
    }

    #[test]
    fn vue27_compile_script_hoists_ts_types_and_runtime_enums() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
export interface Foo {}
type Bar = {}
enum Baz { A = 1 }
const enum Qux { A = 2 }
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        let setup_index = script.content.find("setup(__props)").unwrap();
        assert!(script.content.find("export interface Foo {}").unwrap() < setup_index);
        assert!(script.content.find("type Bar = {}").unwrap() < setup_index);
        assert!(script.content.find("enum Baz { A = 1 }").unwrap() < setup_index);
        assert!(script.content.find("const enum Qux { A = 2 }").unwrap() < setup_index);
        assert!(script.content.contains("return { __sfc: true,Baz, Qux }"));
        assert_eq!(
            script.bindings.get("Baz").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("Qux").map(String::as_str),
            Some("setup-const")
        );
        assert!(!script.bindings.contains_key("Foo"));
        assert!(!script.bindings.contains_key("Bar"));
    }

    #[test]
    fn vue27_compile_script_returns_normal_script_runtime_enums() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script lang="ts">
export enum D { D = "D" }
const enum C { C = "C" }
enum B { B = "B" }
</script>
<script setup lang="ts">
enum Foo { A = 123 }
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("return { __sfc: true,D, C, B, Foo }"));
        for name in ["D", "C", "B", "Foo"] {
            assert_eq!(
                script.bindings.get(name).map(String::as_str),
                Some("setup-const")
            );
        }
    }

    #[test]
    fn vue27_compile_script_infers_setup_component_name_from_filename() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            "<script setup>const a = 1</script><template>{{ a }}</template>",
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("export default {\n  __name: 'FooBar',"));
        assert!(script.content.contains("return { __sfc: true,a }"));
    }

    #[test]
    fn vue27_compile_script_preserves_manual_default_export_name() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script>
export default {
  name: 'Baz'
}
</script>
<script setup>const a = 1</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("const __default__ = {\n  name: 'Baz'"));
        assert!(script
            .content
            .contains("export default /*#__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("__name: 'FooBar'"));
    }

    #[test]
    fn vue27_compile_script_merges_ts_default_export_with_define_component() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">
export default {
  name: 'Baz'
}
</script>
<script setup lang="ts">const a = 1</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("import { defineComponent as _defineComponent } from 'vue'"));
        assert!(script
            .content
            .contains("const __default__ = {\n  name: 'Baz'"));
        assert!(script
            .content
            .contains("export default /*#__PURE__*/_defineComponent({\n  ...__default__,"));
    }

    #[test]
    fn vue27_compile_script_generates_runtime_macro_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const props = defineProps({ foo: String })
const emit = defineEmits(['save'])
defineExpose({ reset() {} })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("setup(__props, { emit, expose })"));
        assert!(script.content.contains("const props = __props;"));
        assert!(script.content.contains("expose({ reset() {} })"));
        assert!(script
            .content
            .contains("return { __sfc: true,props, emit }"));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("defineEmits"));
        assert!(!script.content.contains("defineExpose"));
    }

    #[test]
    fn vue27_compile_script_unbound_define_emits_only_generates_runtime_option() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
defineEmits(['save'])
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("setup(__props)"));
        assert!(!script.content.contains("{ emit }"));
        assert!(!script.content.contains("defineEmits"));
    }

    #[test]
    fn vue27_compile_script_preserves_define_props_binding_pattern_alias() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const { foo, bar: baz } = defineProps({ foo: String, bar: Number })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("const { foo, bar: baz } = __props;"));
        assert!(script.content.contains("return { __sfc: true,foo, baz }"));
        assert!(!script.content.contains("defineProps"));
    }

    #[test]
    fn vue27_compile_script_removes_runtime_macros_from_multi_declaration() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const props = defineProps(['item']),
  a = 1,
  emit = defineEmits(['save'])
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("props: ['item'],"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("const a = 1"));
        assert!(script.content.contains("const props = __props;"));
        assert!(script
            .content
            .contains("return { __sfc: true,props, a, emit }"));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("defineEmits"));
    }

    #[test]
    fn vue27_compile_script_infers_ts_define_props_from_normal_script() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script lang="ts">
export interface Props { x?: number }
</script>
<script setup lang="ts">
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("x: { type: Number, required: false }"));
        assert_eq!(script.bindings.get("x").map(String::as_str), Some("props"));
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_infers_with_defaults_runtime_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  bar?: number;
  baz: boolean;
  qux?(): number
}>(), {
  foo: 'hi',
  qux() { return 1 }
})
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'hi' }"));
        assert!(script
            .content
            .contains("qux: { type: Function, required: false, default() { return 1 } }"));
        assert!(script.content.contains(
            "const props = __props as { foo: string, bar?: number, baz: boolean, qux(): number };"
        ));
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_infers_define_emits_type_and_rejects_union() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
const emit = defineEmits<{(e: 'foo' | 'bar'): void; (e: 'baz', id: number): void;}>()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains(r#"emits: ["foo", "bar", "baz"],"#));
        assert!(script
            .content
            .contains("emit: ({(e: 'foo' | 'bar'): void; (e: 'baz', id: number): void;})"));
        assert!(script.errors.is_empty());

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
const emit = defineEmits<((e: 'foo') => void) | ((e: 'bar') => void)>()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(script.errors.len(), 1);
        assert!(script.errors[0].contains("type argument passed to defineEmits()"));
    }
