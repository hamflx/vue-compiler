    #[test]
    fn vue3_compile_script_reports_normal_script_option_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script>
const ignored = 2
export default {
  props: ['foo', 'bar'],
  inject: { service: {} },
  setup() {
    return { fromSetup: 1 }
  },
  data() {
    return { fromData: null }
  },
  methods: { save() {} },
  computed: {
    total() {},
    named: { get() {}, set() {} }
  }
}
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

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
            script.bindings.get("service").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("fromSetup").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("fromData").map(String::as_str),
            Some("data")
        );
        assert_eq!(
            script.bindings.get("save").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("total").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("named").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("false")
        );
        assert!(script.bindings.get("ignored").is_none());

        let async_descriptor = compiler.parse(
            "Comp.vue",
            r#"<script>
export default {
  async setup() {
    return { asyncSetup: 1 }
  }
}
</script>"#,
        );
        let async_script =
            compiler.compile_script(&async_descriptor, SfcScriptCompileOptions::default());
        assert!(async_script.errors.is_empty(), "{:?}", async_script.errors);
        assert_eq!(
            async_script.bindings.get("asyncSetup").map(String::as_str),
            Some("setup-maybe-ref")
        );
    }

    #[test]
    fn vue3_compile_script_normal_script_only_ignores_call_default_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script>
import { defineComponent } from 'vue'
export default defineComponent({
  props: ['foo'],
  data() {
    return { bar: 1 }
  }
})
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.bindings.is_empty());
    }

    #[test]
    fn vue3_compile_script_merges_normal_script_bindings_with_setup_metadata() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script>
import { ref as r } from 'vue'
export const literal = 2
let count = 0
const objectValue = {}
export default {
  props: { foo: String },
  data() {
    return { dataValue: null }
  },
  methods: { save() {} }
}
</script>
<script setup>
const local = 1
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(
            script.bindings.get("literal").map(String::as_str),
            Some("literal-const")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("objectValue").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("local").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("dataValue").map(String::as_str),
            Some("data")
        );
        assert_eq!(
            script.bindings.get("save").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("r").map(String::as_str),
            Some("setup-const")
        );
        assert!(script.bindings.get("__isScriptSetup").is_none());
        assert!(script
            .content
            .contains("get count() { return count }, set count(v) { count = v }"));
    }

    #[test]
    fn vue3_compile_script_merges_normal_default_export_with_setup() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("__name: 'Comp'"));
        assert!(script
            .content
            .contains("const a = 1\nconst __returned__ = { a }"));
        assert!(!script.content.contains("_defineComponent"));
    }

    #[test]
    fn vue3_compile_script_merges_named_default_export_with_setup() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script>const def = {}; export { def as default }</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("const def = {};"));
        assert!(script.content.contains("const __default__ = def"));
        assert!(!script.content.contains("export {"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(script.content.contains("__name: 'Comp'"));
        assert!(script.content.contains("const __returned__ = { def, a }"));
    }

    #[test]
    fn vue3_compile_script_keeps_normal_script_without_default_in_setup_compile() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script>export const n = 1</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("export const n = 1"));
        assert!(script.content.contains("export default {"));
        assert!(!script.content.contains("const __default__ = {}"));
        assert!(!script.content.contains("Object.assign(__default__"));
        assert!(script.content.contains("const a = 1\nconst __returned__"));
    }

    #[test]
    fn vue3_compile_script_merges_typescript_default_with_define_component() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script lang=\"ts\">export default { name: 'X' }</script><script setup lang=\"ts\">const a: number = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/_defineComponent({\n  ...__default__,"));
        assert!(!script.content.contains("Object.assign(__default__"));
        assert!(script
            .content
            .contains("const a: number = 1\nconst __returned__ = { a }"));
    }

    #[test]
    fn vue3_compile_script_honors_gen_default_as_for_normal_script() {
        let mut compiler = SfcCompiler::new();
        let options = SfcScriptCompileOptions {
            id: Some("xxxxxxxx".into()),
            gen_default_as: Some("_sfc_".into()),
            ..SfcScriptCompileOptions::default()
        };

        let default_descriptor =
            compiler.parse("Comp.vue", "<script>export default { name: 'X' }</script>");
        let script = compiler.compile_script(&default_descriptor, options.clone());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("const _sfc_ = { name: 'X' }"));
        assert!(!script.content.contains("export default"));
        assert!(!script.content.contains("__default__"));

        let no_default = compiler.parse("Comp.vue", "<script>export const n = 1</script>");
        let script = compiler.compile_script(&no_default, options.clone());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("export const n = 1"));
        assert!(script.content.contains("const _sfc_ = {}"));
        assert!(!script.content.contains("export default"));

        let css_vars = compiler.parse(
            "Comp.vue",
            r#"<script>const color = 'red'</script>
<style>div { color: v-bind(color); }</style>"#,
        );
        let script = compiler.compile_script(&css_vars, options);
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("const _sfc_ = {}"));
        assert!(script.content.contains("const __setup__ = _sfc_.setup"));
        assert!(script.content.contains("_sfc_.setup = __setup__"));
        assert!(!script.content.contains("__default__"));
        assert!(!script.content.contains("export default"));
    }

    #[test]
    fn vue3_compile_script_honors_gen_default_as_for_script_setup() {
        let mut compiler = SfcCompiler::new();
        let options = SfcScriptCompileOptions {
            gen_default_as: Some("_sfc_".into()),
            ..SfcScriptCompileOptions::default()
        };

        let setup = compiler.parse("Comp.vue", "<script setup>const a = 1</script>");
        let script = compiler.compile_script(&setup, options.clone());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("const _sfc_ = {"));
        assert!(script.content.contains("setup(__props"));
        assert!(script.content.contains("const __returned__ = { a }"));
        assert!(!script.content.contains("export default"));

        let setup_ts = compiler.parse(
            "Comp.vue",
            "<script setup lang=\"ts\">const a: number = 1</script>",
        );
        let script = compiler.compile_script(&setup_ts, options.clone());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("const _sfc_ = /*@__PURE__*/_defineComponent({"));
        assert!(!script.content.contains("export default"));

        let merged = compiler.parse(
            "Comp.vue",
            "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&merged, options.clone());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("const _sfc_ = /*@__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("export default"));

        let merged_ts = compiler.parse(
            "Comp.vue",
            "<script lang=\"ts\">export default { name: 'X' }</script><script setup lang=\"ts\">const a: number = 1</script>",
        );
        let script = compiler.compile_script(&merged_ts, options);
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("const _sfc_ = /*@__PURE__*/_defineComponent({\n  ...__default__,"));
        assert!(!script.content.contains("export default"));
    }

    #[test]
    fn vue3_compile_script_returns_raw_non_js_lang_without_public_ast() {
        let mut compiler = SfcCompiler::new();

        let normal = compiler.parse("Comp.vue", "<script lang=\"coffee\">x = 1</script>");
        let script = compiler.compile_script(&normal, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(script.content, "x = 1");
        assert!(!script.setup);
        assert_eq!(script.lang.as_deref(), Some("coffee"));
        assert!(script.script_ast.is_empty());
        assert!(script.script_setup_ast.is_empty());
        let script_json = serde_json::to_value(&script).expect("script json");
        assert!(script_json.get("scriptAst").is_none());
        assert!(script_json.get("scriptSetupAst").is_none());

        let setup = compiler.parse("Comp.vue", "<script setup lang=\"coffee\">x = 1</script>");
        let script = compiler.compile_script(&setup, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(script.content, "x = 1");
        assert!(script.setup);
        assert_eq!(script.lang.as_deref(), Some("coffee"));
        assert!(script.script_ast.is_empty());
        assert!(script.script_setup_ast.is_empty());
        let script_json = serde_json::to_value(&script).expect("script json");
        assert!(script_json.get("scriptAst").is_none());
        assert!(script_json.get("scriptSetupAst").is_none());
    }

    #[test]
    fn vue3_compile_script_script_ast_modes_control_public_projection() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script>export default { name: 'X' }</script><script setup>const a = call()</script>"#,
        );

        let full = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert_eq!(full.script_ast.len(), 1);
        assert_eq!(
            full.script_ast[0]["declaration"]["type"],
            json!("ObjectExpression")
        );
        assert_eq!(full.script_setup_ast.len(), 1);
        assert_eq!(
            full.script_setup_ast[0]["declarations"][0]["id"]["name"],
            json!("a")
        );

        let none = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                script_ast_mode: SfcScriptAstMode::None,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(none.script_ast.is_empty());
        assert!(none.script_setup_ast.is_empty());
        let none_json = serde_json::to_value(&none).expect("none script json");
        assert!(none_json.get("scriptAst").is_none());
        assert!(none_json.get("scriptSetupAst").is_none());

        let top_level = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                script_ast_mode: SfcScriptAstMode::TopLevel,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert_eq!(top_level.script_ast.len(), 1);
        assert_eq!(
            top_level.script_ast[0]["type"],
            json!("ExportDefaultDeclaration")
        );
        assert_eq!(
            top_level.script_ast[0]["source"],
            json!("export default { name: 'X' }")
        );
        assert!(top_level.script_ast[0].get("declaration").is_none());
        assert_eq!(top_level.script_setup_ast.len(), 1);
        assert_eq!(
            top_level.script_setup_ast[0]["type"],
            json!("VariableDeclaration")
        );
        assert_eq!(
            top_level.script_setup_ast[0]["source"],
            json!("const a = call()")
        );
        assert!(top_level.script_setup_ast[0].get("declarations").is_none());
    }

    #[test]
    fn script_ast_line_index_matches_position_scan() {
        for source in [
            "",
            "alpha",
            "a\nb",
            "a\rb",
            "a\r\nb",
            "const face = \"\u{1F600}\"\r\nconst name = face\nconst done = true\rface",
        ] {
            let index = SfcScriptLineIndex::new(source);
            for offset in 0..=source.len() {
                assert_eq!(
                    index.position_at(source, offset),
                    position_at(source, offset),
                    "source {source:?} offset {offset}"
                );
            }
        }
    }

    #[test]
    fn vue3_compile_script_script_ast_loc_preserves_utf16_columns() {
        let script_source = "const first = 1\r\nconst emoji = \"\u{1F600}\"\nconst last = emoji";
        let sfc_source = format!("<script setup>{script_source}</script>");
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("Comp.vue", &sfc_source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(script.script_setup_ast.len(), 3);

        let emoji_statement = &script.script_setup_ast[1];
        let emoji_source = "const emoji = \"\u{1F600}\"";
        let emoji_start = script_source.find(emoji_source).expect("emoji statement");
        let emoji_end = emoji_start + emoji_source.len();
        assert_eq!(emoji_statement["source"], json!(emoji_source));
        assert_eq!(emoji_statement["start"], json!(emoji_start));
        assert_eq!(emoji_statement["end"], json!(emoji_end));
        assert_eq!(
            emoji_statement["loc"]["start"],
            json!({ "column": 1, "line": 2, "offset": emoji_start })
        );
        assert_eq!(
            emoji_statement["loc"]["end"],
            json!({
                "column": emoji_source.encode_utf16().count() + 1,
                "line": 2,
                "offset": emoji_end,
            })
        );
    }

    #[test]
    fn vue3_compile_script_reports_language_mismatch_without_parse_noise() {
        let mut compiler = SfcCompiler::new();
        let message = "<script> and <script setup> must have the same language type.";

        let setup_js = compiler.parse(
            "Comp.vue",
            "<script>foo()</script><script setup lang=\"js\">bar()</script>",
        );
        let script = compiler.compile_script(&setup_js, SfcScriptCompileOptions::default());
        assert_eq!(script.errors, vec![message.to_string()]);
        assert_eq!(script.content, "foo()\nbar()");

        let normal_js = compiler.parse(
            "Comp.vue",
            "<script lang=\"js\">foo()</script><script setup>bar()</script>",
        );
        let script = compiler.compile_script(&normal_js, SfcScriptCompileOptions::default());
        assert_eq!(script.errors, vec![message.to_string()]);
        assert_eq!(script.content, "foo()\nbar()");

        let matching_js = compiler.parse(
            "Comp.vue",
            "<script lang=\"js\">export default {}</script><script setup lang=\"js\">const a = 1</script>",
        );
        let script = compiler.compile_script(&matching_js, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("export default"));
    }

    #[test]
    fn vue3_compile_script_reports_script_setup_value_exports() {
        let mut compiler = SfcCompiler::new();

        let value_export = compiler.parse("Comp.vue", "<script setup>export const a = 1</script>");
        let script = compiler.compile_script(&value_export, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 1);
        assert!(script.errors[0].contains("cannot contain ES module exports"));

        let default_export = compiler.parse("Comp.vue", "<script setup>export default {}</script>");
        let script = compiler.compile_script(&default_export, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 1);
        assert!(script.errors[0].contains("cannot contain ES module exports"));

        let type_export = compiler.parse(
            "Comp.vue",
            "<script setup lang=\"ts\">type Foo = string\nexport type { Foo }</script>",
        );
        let script = compiler.compile_script(&type_export, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
    }

    #[test]
    fn vue3_compile_script_import_attributes_honor_deprecated_assert_option() {
        let mut compiler = SfcCompiler::new();

        let with_syntax = compiler.parse(
            "Comp.vue",
            "<script setup>import { foo } from './foo.js' with { type: 'json' }</script>",
        );
        let script = compiler.compile_script(&with_syntax, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("import { foo } from './foo.js' with { type: 'json' }"));

        let assert_syntax = compiler.parse(
            "Comp.vue",
            "<script setup>import { foo } from './foo.js' assert { type: 'json' }</script>",
        );
        let script = compiler.compile_script(&assert_syntax, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("`assert` keyword in import attributes is deprecated")
        }));

        let script = compiler.compile_script(
            &assert_syntax,
            SfcScriptCompileOptions {
                allow_deprecated_import_assert_syntax: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("import { foo } from './foo.js' assert { type: 'json' }"));
    }

    #[test]
    fn vue3_compile_script_injects_normal_script_css_vars() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script>const a = 1</script>
<style>
div {
  color: v-bind(color);
  font-size: v-bind('font.size');
}
</style>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("const a = 1"));
        assert!(script.content.contains("const __default__ = {}"));
        assert!(script
            .content
            .contains("import { useCssVars as _useCssVars } from 'vue'"));
        assert!(script.content.contains("_useCssVars(_ctx => ({"));
        assert!(script.content.contains(r#""xxxxxxxx-color": (_ctx.color)"#));
        assert!(script
            .content
            .contains(r#""xxxxxxxx-font\.size": (_ctx.font.size)"#));
        assert!(script.content.contains("export default __default__"));
    }

    #[test]
    fn vue3_compile_script_injects_script_setup_css_vars() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup>
import { ref } from 'vue'
const color = 'red'
const size = ref('10px')
defineProps({ foo: String })
</script>
<style>
div {
  color: v-bind(color);
  font-size: v-bind(size);
  border: v-bind(foo);
}
</style>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("import { useCssVars as _useCssVars, unref as _unref } from 'vue'"));
        assert!(script.content.contains("_useCssVars(_ctx => ({"));
        assert!(script.content.contains(r#""xxxxxxxx-color": (color)"#));
        assert!(script.content.contains(r#""xxxxxxxx-size": (size.value)"#));
        assert!(script.content.contains(r#""xxxxxxxx-foo": (__props.foo)"#));
        assert!(!script.content.contains(r#""xxxxxxxx-ignored": (ignored)"#));
    }

    #[test]
    fn vue3_compile_script_can_omit_script_setup_marker_for_official_tests() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup>const color = 'red'</script>
<style>div { color: v-bind(color); }</style>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("_useCssVars(_ctx => ({"));
        assert!(script.content.contains(r#""xxxxxxxx-color": (color)"#));
        assert!(script.content.contains("return { color }"));
        assert!(!script
            .content
            .contains("Object.defineProperty(__returned__"));
    }

    #[test]
    fn vue3_compile_script_does_not_infer_name_for_default_filename() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "anonymous.vue",
            r#"<script setup>const color = 'red'</script>
<style>div { color: v-bind(color); }</style>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("export default {\n  setup("));
        assert!(!script.content.contains("__name: 'anonymous'"));
        assert!(!script.content.contains("__name:"));
    }

    #[test]
    fn vue3_compile_script_matches_public_macro_snapshot_spacing() {
        let mut compiler = SfcCompiler::new();
        let define_expose = compiler.parse(
            "anonymous.vue",
            r#"
<script setup>
defineExpose({ foo: 123 })
</script>
"#,
        );
        let script = compiler.compile_script(
            &define_expose,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.starts_with("\nexport default {"));
        assert!(script
            .content
            .contains("__expose({ foo: 123 })\n\nreturn {  }"));

        let define_options = compiler.parse(
            "anonymous.vue",
            r#"
      <script setup>
      defineOptions({ name: 'FooApp' })
      </script>
    "#,
        );
        let script = compiler.compile_script(
            &define_options,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("\nexport default /*@__PURE__*/Object.assign({ name: 'FooApp' }, {"));
        assert!(script
            .content
            .contains("  __expose();\n\n      \n      \nreturn {  }"));

        let script_after_setup = compiler.parse(
            "anonymous.vue",
            r#"
  <script setup>
  import { x } from './x'
  </script>
  <script>const n = 1</script>
  "#,
        );
        let script = compiler.compile_script(
            &script_after_setup,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { x } from './x'\n  const n = 1\n\nexport default {"));
        assert!(script
            .content
            .contains("return { n, get x() { return x } }"));
    }

    #[test]
    fn vue3_compile_script_css_vars_skip_line_comments_and_ssr() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup>const color = 'red'; const width = 100</script>
<style lang="scss">
// div { color: v-bind(color); }
div { width: v-bind(width); }
</style>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(!script.content.contains(r#""xxxxxxxx-color""#));
        assert!(script.content.contains(r#""xxxxxxxx-width": (width)"#));

        let ssr = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                inline_template_ssr: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(ssr.errors.is_empty(), "{:?}", ssr.errors);
        assert!(!ssr.content.contains("_useCssVars"));
    }

    #[test]
    fn vue3_compile_script_generates_runtime_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const props = defineProps({ foo: String })
const emit = defineEmits(['save'])
defineExpose({ reset() {} })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("__name: 'FooBar',"));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script
            .content
            .contains("setup(__props, { expose: __expose, emit: __emit })"));
        assert!(script.content.contains("const props = __props"));
        assert!(script.content.contains("const emit = __emit"));
        assert!(script.content.contains("__expose({ reset() {} })"));
        assert!(script
            .content
            .contains("const __returned__ = { props, emit }"));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("defineEmits"));
        assert!(!script.content.contains("defineExpose"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
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
    fn vue3_compile_script_rewrites_define_slots() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { defineSlots } from 'vue'
const slots = defineSlots<{
  default: { msg: string }
}>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains(
            "import { useSlots as _useSlots, defineComponent as _defineComponent } from 'vue'"
        ));
        assert!(script.content.contains("const slots = _useSlots()"));
        assert!(script.content.contains("const __returned__ = { slots }"));
        assert!(!script.content.contains("defineSlots"));
        assert_eq!(
            script.bindings.get("slots").map(String::as_str),
            Some("setup-const")
        );
        assert!(script.bindings.get("defineSlots").is_none());

        let unbound = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
defineSlots<{
  default: { msg: string }
}>()
</script>"#,
        );
        let script = compiler.compile_script(&unbound, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(!script.content.contains("defineSlots"));
        assert!(!script.content.contains("_useSlots"));

        let runtime = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const slots = defineSlots()
</script>"#,
        );
        let script = compiler.compile_script(&runtime, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { useSlots as _useSlots } from 'vue'"));
        assert!(script.content.contains("const slots = _useSlots()"));
        assert!(!script.content.contains("defineSlots"));
    }

    #[test]
    fn vue3_compile_script_reports_define_slots_errors() {
        let mut compiler = SfcCompiler::new();
        let duplicate = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineSlots()
defineSlots()
</script>"#,
        );
        let script = compiler.compile_script(&duplicate, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineSlots() call")));
        assert!(!script.content.contains("defineSlots"));
        assert!(!script.content.contains("_useSlots"));

        let arguments = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const slots = defineSlots({})
</script>"#,
        );
        let script = compiler.compile_script(&arguments, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("defineSlots() cannot accept arguments")));
        assert!(script.content.contains("const slots = _useSlots()"));
        assert!(!script.content.contains("defineSlots"));
    }

    #[test]
    fn vue3_compile_script_rewrites_top_level_await() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const a = 1 + (await foo)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { withAsyncContext as _withAsyncContext } from 'vue'\n"));
        assert!(script.content.contains("async setup("));
        assert!(script.content.contains("let __temp, __restore\n"));
        assert!(script
            .content
            .contains("([__temp,__restore] = _withAsyncContext(() => foo))"));
        assert!(script.content.contains("__temp = await __temp"));
        assert!(script.content.contains("__restore(),\n  __temp"));
        assert!(script.content.contains("const __returned__ = { a }"));
    }

    #[test]
    fn vue3_compile_script_top_level_await_ignores_function_scopes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
async function foo() { await bar }
const fn = async () => { await bar }
const obj = { async method() { await bar }}
const cls = class Foo { async method() { await bar } }
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(!script.content.contains("_withAsyncContext"));
        assert!(!script.content.contains("async setup("));
        assert!(!script.content.contains("let __temp"));
        assert!(script
            .content
            .contains("async function foo() { await bar }"));
        assert!(script
            .content
            .contains("const obj = { async method() { await bar }}"));
        assert!(script
            .content
            .contains("const cls = class Foo { async method() { await bar } }"));
    }

    #[test]
    fn vue3_compile_script_top_level_await_handles_nested_and_semicolon() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
foo()
await 1 + await 2
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("foo()\n;("));
        assert!(script.content.matches("_withAsyncContext").count() >= 2);

        let nested = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
await (await foo)
</script>"#,
        );
        let script = compiler.compile_script(&nested, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("_withAsyncContext(async () => ("));
        assert!(script.content.matches("_withAsyncContext").count() >= 2);
    }
