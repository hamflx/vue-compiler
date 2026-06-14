    #[test]
    fn vue27_bridge_compile_style_rewrites_css_vars_with_default_scope() {
        let compiled = dispatch(
            "sfc.vue27.compileStyle",
            json!({
                "source": ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
                "filename": "test.css",
                "options": {
                    "id": "data-v-test"
                }
            }),
        )
        .expect("vue27 style");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(".foo[data-v-test]"));
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_style_compiles_raw_css_source() {
        let compiled = dispatch(
            "sfc.compileStyle",
            json!({
                "source": ".foo { color: red; }",
                "filename": "test.css",
                "options": {
                    "id": "data-v-test",
                    "scoped": true
                }
            }),
        )
        .expect("vue3 style");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(".foo[data-v-test] { color: red;"));
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert_eq!(compiled["rawResult"], json!(["postcss-result"]));

        let modules = dispatch(
            "sfc.compileStyleAsync",
            json!({
                "source": ".red { color: red; } :global(.blue) { color: blue; }",
                "filename": "test.css",
                "options": {
                    "id": "test",
                    "modules": true
                }
            }),
        )
        .expect("vue3 style modules");

        assert!(modules["modules"]["red"]
            .as_str()
            .unwrap_or("")
            .contains("_red_"));
        assert!(modules["modules"].get("blue").is_none());
    }

    #[test]
    fn vue27_bridge_parse_collects_comment_separated_css_vars() {
        let parsed = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": r#"<style>.foo { color: v-bind/**/(color); font-size: v-bind /*x*/ ('font.size'); }</style>"#,
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse");

        assert_eq!(parsed["cssVars"], json!(["color", "font.size"]));
    }

    #[test]
    fn vue27_bridge_parse_uses_legacy_deindent() {
        let parsed = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": "<template>\n  <div id=\"app\">\n    <router-view />\n  </div>\n</template>",
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse");

        assert_eq!(
            parsed["template"]["content"],
            json!("\n<div id=\"app\">\n  <router-view />\n</div>\n")
        );
    }

    #[test]
    fn vue27_bridge_parse_projects_errors_by_source_range_option() {
        let source = r#"<template>
<div>
  <input>
</div>
</template>"#;
        let default = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": source,
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse default errors");
        assert_eq!(
            default["errors"],
            json!(["tag <input> has no matching end tag."])
        );

        let ranged = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": source,
                "filename": "test.vue",
                "options": {
                    "outputSourceRange": true
                }
            }),
        )
        .expect("vue27 parse ranged errors");
        let errors = ranged["errors"].as_array().expect("ranged errors");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0]["msg"],
            json!("tag <input> has no matching end tag.")
        );
        assert!(errors[0]["start"].as_u64().is_some());
        assert!(errors[0]["end"].as_u64().is_some());
    }

    #[test]
    fn vue3_sfc_bridge_rewrite_default_routes_parser_plugins() {
        let rewritten = dispatch(
            "sfc.rewriteDefault",
            json!({
                "source": "export { foo as default, bar } from './index.js'",
                "variable": "script",
                "plugins": []
            }),
        )
        .expect("vue3 rewriteDefault");
        assert_eq!(
            rewritten,
            json!("import { foo as __VUE_DEFAULT__ } from './index.js'\nexport {  bar } from './index.js'\nconst script = __VUE_DEFAULT__")
        );

        let without_ts = dispatch(
            "sfc.rewriteDefault",
            json!({
                "source": "export default interface Foo {}",
                "variable": "__default__",
                "plugins": []
            }),
        )
        .unwrap_err();
        assert!(format!("{without_ts:#}").contains("Unexpected reserved word 'interface'. (1:15)"));

        let with_ts = dispatch(
            "sfc.rewriteDefault",
            json!({
                "source": "export default interface Foo {}",
                "variable": "__default__",
                "plugins": [["typescript", {}]]
            }),
        )
        .expect("vue3 TypeScript rewriteDefault");
        assert_eq!(with_ts, json!("const __default__ = interface Foo {}"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_normal_default_export_with_setup() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("const __default__ = { name: 'X' }"));
        assert!(content.contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(content.contains("const a = 1\nconst __returned__ = { a }"));

        let script_ast = compiled["scriptAst"].as_array().expect("scriptAst array");
        assert_eq!(script_ast.len(), 1);
        assert_eq!(script_ast[0]["type"], json!("ExportDefaultDeclaration"));
        assert_eq!(
            script_ast[0]["source"],
            json!("export default { name: 'X' }")
        );
        assert_eq!(
            script_ast[0]["declaration"]["type"],
            json!("ObjectExpression")
        );
        assert_eq!(script_ast[0]["loc"]["start"]["offset"], json!(0));

        let setup_ast = compiled["scriptSetupAst"]
            .as_array()
            .expect("scriptSetupAst array");
        assert_eq!(setup_ast.len(), 1);
        assert_eq!(setup_ast[0]["type"], json!("VariableDeclaration"));
        assert_eq!(setup_ast[0]["kind"], json!("const"));
        assert_eq!(setup_ast[0]["source"], json!("const a = 1"));
        assert_eq!(setup_ast[0]["declarations"][0]["id"]["name"], json!("a"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_internal_script_ast_mode() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
                "filename": "Comp.vue",
                "options": {
                    "__vuecScriptAstMode": "none"
                }
            }),
        )
        .expect("vue3 compileScript");

        assert!(compiled.get("scriptAst").is_none());
        assert!(compiled.get("scriptSetupAst").is_none());

        let top_level = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script>",
                "filename": "Comp.vue",
                "options": {
                    "scriptAstMode": "top-level"
                }
            }),
        )
        .expect("vue3 compileScript top-level AST");
        let script_ast = top_level["scriptAst"].as_array().expect("scriptAst array");
        assert_eq!(script_ast[0]["type"], json!("ExportDefaultDeclaration"));
        assert!(script_ast[0].get("declaration").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_gen_default_as_option() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>const a = 1</script>",
                "filename": "Comp.vue",
                "options": {
                    "genDefaultAs": "_sfc_"
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("const _sfc_ = {"));
        assert!(!content.contains("export default"));

        let snake_case = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script>",
                "filename": "Comp.vue",
                "options": {
                    "gen_default_as": "_sfc_"
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = snake_case["content"].as_str().unwrap_or_default();
        assert!(content.contains("const _sfc_ = { name: 'X' }"));
        assert!(!content.contains("export default"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_import_attributes_parser_option() {
        let with_syntax = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>import { foo } from './foo.js' with { type: 'json' }</script>",
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(with_syntax["errors"].as_array().unwrap().is_empty());

        let assert_syntax = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>import { foo } from './foo.js' assert { type: 'json' }</script>",
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(assert_syntax["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error
                    .as_str()
                    .is_some_and(|error| error.contains("import attributes is deprecated"))
            }));

        let overridden = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>import { foo } from './foo.js' assert { type: 'json' }</script>",
                "filename": "Comp.vue",
                "options": {
                    "babelParserPlugins": [
                        ["importAttributes", { "deprecatedAssertSyntax": true }]
                    ]
                }
            }),
        )
        .expect("vue3 compileScript");
        assert!(overridden["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_projects_source_map_option() {
        let source = "<script setup>\nconst count = 1\n</script>";
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": source,
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        assert_eq!(compiled["map"]["version"], json!(3));
        assert_eq!(compiled["map"]["sources"], json!(["FooBar.vue"]));
        assert_eq!(compiled["map"]["sourcesContent"][0], json!(source));
        assert!(compiled["map"]["mappings"]
            .as_str()
            .is_some_and(|mappings| !mappings.is_empty()));

        let disabled = dispatch(
            "sfc.compileScript",
            json!({
                "source": source,
                "filename": "FooBar.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 compileScript");
        assert!(disabled["map"].is_null());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_normal_script_bindings() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script>",
                    "const ignored = 1\n",
                    "export default {",
                    "props: ['foo'],",
                    "inject: { service: {} },",
                    "data() { return { count: 1 } },",
                    "methods: { save() {} }",
                    "}",
                    "</script>"
                ),
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");

        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["service"], json!("options"));
        assert_eq!(compiled["bindings"]["count"], json!("data"));
        assert_eq!(compiled["bindings"]["save"], json!("options"));
        assert_eq!(compiled["bindings"]["__isScriptSetup"], json!("false"));
        assert!(compiled["bindings"].get("ignored").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_generates_runtime_macros() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const props = defineProps({ foo: String })\n",
                    "const emit = defineEmits(['save'])\n",
                    "defineExpose({ reset() {} })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("props: { foo: String },"));
        assert!(content.contains("emits: ['save'],"));
        assert!(content.contains("setup(__props, { expose: __expose, emit: __emit })"));
        assert!(content.contains("const props = __props"));
        assert!(content.contains("const emit = __emit"));
        assert!(content.contains("__expose({ reset() {} })"));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["props"], json!("setup-reactive-const"));
        assert_eq!(compiled["bindings"]["emit"], json!("setup-const"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_rewrites_define_slots() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { defineSlots, ref } from 'vue'\n",
                    "const slots = defineSlots<{ default: { msg: string } }>()\n",
                    "const count = ref(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(
            "import { useSlots as _useSlots, defineComponent as _defineComponent } from 'vue'"
        ));
        assert!(content.contains("import { ref } from 'vue'"));
        assert!(content.contains("const slots = _useSlots()"));
        assert!(content.contains("const __returned__ = { slots, count, ref }"));
        assert!(!content.contains("defineSlots"));
        assert_eq!(compiled["bindings"]["slots"], json!("setup-const"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert!(compiled["bindings"].get("defineSlots").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_duplicate_define_expose() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "defineExpose({ first: true })\n",
                    "defineExpose({ second: true })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().iter().any(|error| {
            error
                .as_str()
                .is_some_and(|error| error.contains("duplicate defineExpose() call"))
        }));
        assert!(content.contains("__expose({ first: true })"));
        assert!(content.contains("__expose({ second: true })"));
        assert!(!content.contains("defineExpose"));
        assert!(!content.contains("__expose();"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_infers_typescript_macros() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "type Props = { foo?: string; ok?: boolean; cb?: () => void }\n",
                    "const props = withDefaults(defineProps<Props>(), { foo: 'x', ok: true })\n",
                    "const emit = defineEmits<{(e: 'save'): void}>()",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "isProd": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("foo: { default: 'x' }"));
        assert!(content.contains("ok: { type: Boolean, default: true }"));
        assert!(content.contains("cb: {}"));
        assert!(content.contains(r#"emits: ["save"],"#));
        assert!(content.contains("setup(__props: any, { expose: __expose, emit: __emit })"));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["ok"], json!("props"));
        assert_eq!(compiled["bindings"]["cb"], json!("props"));
        assert_eq!(compiled["bindings"]["props"], json!("setup-const"));
        assert_eq!(compiled["bindings"]["emit"], json!("setup-const"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_passes_custom_element_prod_option() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "withDefaults(defineProps<{ foo?: number; bar?: string }>(), { foo: 5.5 })",
                    "</script>"
                ),
                "filename": "Foo.ce.vue",
                "options": {
                    "isProd": true,
                    "customElement": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("foo: { default: 5.5, type: Number }"));
        assert!(content.contains("bar: {type: String}"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_with_defaults_errors() {
        let bad_first = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const props = withDefaults(foo(), { foo: 'x' })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(bad_first["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error
                    .contains("withDefaults' first argument must be a defineProps call"))));
        assert!(!bad_first["content"]
            .as_str()
            .unwrap_or_default()
            .contains("withDefaults"));

        let runtime_props = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const props = withDefaults(defineProps({ foo: String }), { foo: 'x' })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let runtime_content = runtime_props["content"].as_str().unwrap_or_default();
        assert!(runtime_props["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "withDefaults can only be used with type-based defineProps declaration"
            ))));
        assert!(runtime_content.contains("props: { foo: String },"));
        assert!(!runtime_content.contains("withDefaults"));
        assert!(!runtime_content.contains("defineProps"));

        let destructure = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo } = withDefaults(defineProps<{ foo: string }>(), { foo: 'foo' })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let destructure_content = destructure["content"].as_str().unwrap_or_default();
        assert!(destructure["errors"].as_array().unwrap().is_empty());
        assert!(destructure["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .is_some_and(|warning| warning
                    .contains("withDefaults() is unnecessary when using destructure"))));
        assert!(destructure_content.contains("const { foo } = __props"));
        assert_eq!(destructure["bindings"]["foo"], json!("setup-const"));

        let missing_defaults = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const props = withDefaults(defineProps<{ foo?: string }>())",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(missing_defaults["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(
                |error| error.contains("The 2nd argument of withDefaults is required")
            )));
        assert!(!missing_defaults["content"]
            .as_str()
            .unwrap_or_default()
            .contains("withDefaults"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_duplicate_props_and_emits() {
        let duplicate_props = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<{ foo?: string }>()\n",
                    "const props = withDefaults(defineProps<{ bar?: number }>(), { bar: 1 })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let props_content = duplicate_props["content"].as_str().unwrap_or_default();
        assert!(duplicate_props["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("duplicate defineProps() call"))));
        assert!(!props_content.contains("defineProps"));
        assert!(!props_content.contains("withDefaults"));

        let duplicate_emits = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "defineEmits(['save'])\n",
                    "const emit = defineEmits(['cancel'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let emits_content = duplicate_emits["content"].as_str().unwrap_or_default();
        assert!(duplicate_emits["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("duplicate defineEmits() call"))));
        assert!(emits_content.contains("const emit = __emit"));
        assert!(!emits_content.contains("defineEmits"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_type_resolution_errors() {
        let unresolved = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<X>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(unresolved["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error.as_str().is_some_and(|error| {
                    error.contains(
                        "Unresolvable type reference or unsupported built-in utility type",
                    )
                })
            }));

        let missing_import = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { X } from './foo'\n",
                    "defineProps<X>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(missing_import["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error.as_str().is_some_and(|error| {
                    error.contains("Failed to resolve import source \"./foo\".")
                })
            }));

        let silent_member = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type P from 'unknown'\n",
                    "defineProps<{ foo: T, bar: T['bar'], baz: P }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(silent_member["errors"].as_array().unwrap().is_empty());
        assert_eq!(silent_member["bindings"]["foo"], json!("props"));
        assert_eq!(silent_member["bindings"]["bar"], json!("props"));
        assert_eq!(silent_member["bindings"]["baz"], json!("props"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_destructure_errors() {
        let dynamic_key = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const key = 'foo'\n",
                    "const { [key]: foo } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(dynamic_key["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("destructure cannot use computed key"))));

        let nested_pattern = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo: { bar } } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(nested_pattern["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(
                |error| error.contains("destructure does not support nested patterns")
            )));

        let local_default = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "let x = 1\n",
                    "const { foo = () => x } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(local_default["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(
                |error| error.contains("cannot reference locally declared variables")
            )));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_props_destructure_option() {
        let disabled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo, bar: baz } = defineProps(['foo', 'bar'])\n",
                    "const message = foo + baz",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "propsDestructure": false
                }
            }),
        )
        .expect("vue3 compileScript");
        let content = disabled["content"].as_str().unwrap_or_default();
        assert!(disabled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("const { foo, bar: baz } = __props"));
        assert!(content.contains("const message = foo + baz"));
        assert!(!content.contains("__props.foo + __props.bar"));
        assert_eq!(disabled["bindings"]["foo"], json!("setup-const"));
        assert_eq!(disabled["bindings"]["baz"], json!("setup-const"));

        let errored = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "propsDestructure": "error"
                }
            }),
        )
        .expect("vue3 compileScript");
        assert!(errored["errors"].as_array().unwrap().iter().any(|error| {
            error.as_str().is_some_and(|error| {
                error.contains("Props destructure is explicitly prohibited via config.")
            })
        }));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_destructure_usage_errors() {
        let assignment = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo } = defineProps(['foo'])\n",
                    "foo = 'bar'",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(assignment["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("Cannot assign to destructured props"))));

        let watch_alias = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { watch as w, toRef as r } from 'vue'\n",
                    "const { foo, bar } = defineProps(['foo', 'bar'])\n",
                    "w(foo, () => {})\n",
                    "r(bar)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let errors = watch_alias["errors"].as_array().unwrap();
        assert!(errors
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch()."
            ))));
        assert!(errors
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "\"bar\" is a destructured prop and should not be passed directly to toRef()."
            ))));

        let normal_script_watch_alias = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script>",
                    "import { watch as w } from 'vue'",
                    "</script>",
                    "<script setup>",
                    "const { foo } = defineProps(['foo'])\n",
                    "w(foo, () => {})",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(normal_script_watch_alias["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch()."
            ))));

        let shadowed = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { watch } from 'vue'\n",
                    "const { foo } = defineProps(['foo'])\n",
                    "function useLocal(foo) { watch(foo, () => {}); foo++ }",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(shadowed["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_rewrites_props_destructure_references() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo, bar: baz, 'foo.bar': fooBar } = defineProps({ foo: String, bar: Number, 'foo.bar': Boolean })\n",
                    "const message = foo + baz\n",
                    "const payload = { foo, baz, fooBar }\n",
                    "function read(foo) { return foo + baz }\n",
                    "console.log(message, payload, fooBar)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(!content.contains("const { foo, bar: baz, 'foo.bar': fooBar }"));
        assert!(content.contains("const message = __props.foo + __props.bar"));
        assert!(content.contains(
            r#"const payload = { foo: __props.foo, baz: __props.bar, fooBar: __props["foo.bar"] }"#
        ));
        assert!(content.contains("function read(foo) { return foo + __props.bar }"));
        assert!(content.contains(r#"console.log(message, payload, __props["foo.bar"])"#));
        assert_eq!(compiled["propsAliases"]["baz"], json!("bar"));
        assert_eq!(compiled["propsAliases"]["fooBar"], json!("foo.bar"));
        assert!(compiled["propsAliases"].get("foo").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_generates_props_destructure_rest_proxy() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo, bar: baz, ...rest } = defineProps(['foo', 'bar', 'baz'])\n",
                    "const read = foo + baz + rest.baz",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(r#"const rest = _createPropsRestProxy(__props, ["foo","bar"])"#));
        assert!(content.contains("const read = __props.foo + __props.bar + rest.baz"));
        assert!(!content.contains("const { foo, bar: baz, ...rest }"));
        assert!(!content.contains("defineProps"));
        assert_eq!(
            compiled["bindings"]["rest"].as_str(),
            Some("setup-reactive-const")
        );
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_inlines_template_render() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { ref } from 'vue'\n",
                    "import ChildComp from './ChildComp.vue'\n",
                    "const count = ref(0)\n",
                    "const { title: heading } = defineProps(['title'])",
                    "</script>",
                    "<template><div>{{ count }} {{ heading }}</div><ChildComp /></template>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "inlineTemplate": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("toDisplayString as _toDisplayString"));
        assert!(content.contains("return (_ctx, _cache) => {"));
        assert!(content.contains("count.value"));
        assert!(content.contains("_toDisplayString(__props.title)"));
        assert!(content.contains("_createVNode(ChildComp)"));
        assert!(!content.contains("const __returned__"));
        assert_eq!(compiled["bindings"]["heading"], json!("props-aliased"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_inlines_ssr_template_render() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { ref } from 'vue'\n",
                    "const count = ref(0)",
                    "</script>",
                    "<template><div>{{ count }}</div></template>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "id": "xxxxxxxx",
                    "inlineTemplate": true,
                    "templateOptions": {
                        "ssr": true
                    }
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(content.contains("__ssrInlineRender: true,"));
        assert!(content.contains("return (_ctx, _push, _parent, _attrs) => {"));
        assert!(content.contains("_ssrInterpolate(count.value)"));
        assert!(!content.contains("const __returned__"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_rewrites_top_level_await_runtime_module() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const value = await Promise.resolve(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "templateOptions": {
                        "compilerOptions": {
                            "runtimeModuleName": "npm:vue"
                        }
                    }
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content
            .starts_with("import { withAsyncContext as _withAsyncContext } from \"npm:vue\"\n"));
        assert!(content.contains("async setup("));
        assert!(content.contains("let __temp, __restore"));
        assert!(content.contains("_withAsyncContext(() => Promise.resolve(1))"));
        assert!(content.contains("const __returned__ = { value }"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_template_used_import_getters() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { FooBar, FooBaz, vMyDir } from './x'\n",
                    "import { ref } from 'vue'\n",
                    "const local = ref(0)",
                    "</script>",
                    "<template>",
                    "<FooBaz />",
                    "<foo-bar />",
                    "<div v-my-dir>{{ FooBar }}</div>",
                    "</template>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(
            "const __returned__ = { local, get FooBar() { return FooBar }, get FooBaz() { return FooBaz }, get vMyDir() { return vMyDir }, ref }"
        ));
        assert_eq!(compiled["bindings"]["FooBar"], json!("setup-maybe-ref"));
        assert_eq!(compiled["bindings"]["FooBaz"], json!("setup-maybe-ref"));
        assert_eq!(compiled["bindings"]["vMyDir"], json!("setup-maybe-ref"));
        assert_eq!(compiled["bindings"]["ref"], json!("setup-const"));
        assert_eq!(compiled["bindings"]["local"], json!("setup-ref"));
        assert_eq!(compiled["imports"]["FooBar"]["imported"], json!("FooBar"));
        assert_eq!(compiled["imports"]["FooBar"]["local"], json!("FooBar"));
        assert_eq!(compiled["imports"]["FooBar"]["source"], json!("./x"));
        assert_eq!(compiled["imports"]["FooBar"]["isType"], json!(false));
        assert_eq!(compiled["imports"]["FooBar"]["isFromSetup"], json!(true));
        assert_eq!(
            compiled["imports"]["FooBar"]["isUsedInTemplate"],
            json!(true)
        );
        assert_eq!(compiled["imports"]["FooBaz"]["imported"], json!("FooBaz"));
        assert_eq!(compiled["imports"]["FooBaz"]["local"], json!("FooBaz"));
        assert_eq!(compiled["imports"]["FooBaz"]["source"], json!("./x"));
        assert_eq!(compiled["imports"]["FooBaz"]["isType"], json!(false));
        assert_eq!(compiled["imports"]["FooBaz"]["isFromSetup"], json!(true));
        assert_eq!(
            compiled["imports"]["FooBaz"]["isUsedInTemplate"],
            json!(true)
        );
        assert_eq!(
            compiled["imports"]["vMyDir"]["isUsedInTemplate"],
            json!(true)
        );
        assert_eq!(compiled["imports"]["ref"]["source"], json!("vue"));
        assert_eq!(compiled["imports"]["ref"]["isFromSetup"], json!(true));
        assert_eq!(compiled["imports"]["ref"]["isUsedInTemplate"], json!(false));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_props_destructure_defaults() {
        let runtime = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const external = 'x'\n",
                    "const { foo = 1, bar = {}, func = () => {}, ext = external, 'foo:bar': fooBar = 'foo-bar' } = defineProps(['foo', 'bar', 'func', 'ext', 'foo:bar'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let content = runtime["content"].as_str().unwrap_or_default();
        assert!(runtime["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(
            "props: /*@__PURE__*/_mergeDefaults(['foo', 'bar', 'func', 'ext', 'foo:bar'], {"
        ));
        assert!(content.contains("bar: () => ({})"));
        assert!(content.contains("func: () => {}, __skip_func: true"));
        assert!(content.contains("ext: external, __skip_ext: true"));
        assert!(content.contains(r#""foo:bar": 'foo-bar'"#));

        let typed = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo = 1, bar = {}, func = () => {} } = defineProps<{ foo?: number, bar?: object, func?: () => void }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let content = typed["content"].as_str().unwrap_or_default();
        assert!(typed["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: Number, required: false, default: 1 }"));
        assert!(content.contains("bar: { type: Object, required: false, default: () => ({}) }"));
        assert!(content.contains("func: { type: Function, required: false, default: () => {} }"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_destructure_default_type_errors() {
        let mismatch = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo = 'hello' } = defineProps<{ foo?: number }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(mismatch["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error
                .contains("Default value of prop \"foo\" does not match declared type."))));

        let matching = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo = 1, bar = 'ok' } = defineProps<{ foo?: number, bar?: string }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(matching["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_define_options() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { defineOptions, ref } from 'vue'\n",
                    "defineOptions({ name: 'FooApp', inheritAttrs: false })\n",
                    "const count = ref(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("import { ref } from 'vue'"));
        assert!(content.contains(
            "export default /*@__PURE__*/Object.assign({ name: 'FooApp', inheritAttrs: false }, {"
        ));
        assert!(content.contains("__name: 'FooBar',"));
        assert!(content.contains("const __returned__ = { count, ref }"));
        assert!(!content.contains("defineOptions"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert!(compiled["bindings"].get("defineOptions").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_generates_define_model() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { defineModel, ref } from 'vue'\n",
                    "defineProps({ foo: String })\n",
                    "defineEmits(['change'])\n",
                    "const count = defineModel({ default: 0 })\n",
                    "const title = defineModel('title')\n",
                    "const other = ref(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content
            .contains("import { useModel as _useModel, mergeModels as _mergeModels } from 'vue'"));
        assert!(content.contains("import { ref } from 'vue'"));
        assert!(content.contains("props: /*@__PURE__*/_mergeModels({ foo: String }, {"));
        assert!(content.contains("\"modelValue\": { default: 0 },"));
        assert!(content.contains("\"title\": {},"));
        assert!(content.contains(
            "emits: /*@__PURE__*/_mergeModels(['change'], [\"update:modelValue\", \"update:title\"]),"
        ));
        assert!(content.contains(r#"const count = _useModel(__props, "modelValue")"#));
        assert!(content.contains("const title = _useModel(__props, 'title')"));
        assert!(!content.contains("defineModel"));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["modelValue"], json!("props"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["title"], json!("setup-ref"));
        assert!(compiled["bindings"].get("defineModel").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_infers_define_model_types() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const modelValue = defineModel<boolean | string>()\n",
                    "const count = defineModel<number>('count')\n",
                    "const any = defineModel<any | boolean>('any')",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(content.contains("\"count\": { type: Number },"));
        assert!(content.contains("\"any\": { type: Boolean, skipCheck: true },"));
        assert!(
            content.contains("emits: [\"update:modelValue\", \"update:count\", \"update:any\"],")
        );
        assert!(content
            .contains(r#"const modelValue = _useModel<boolean | string>(__props, "modelValue")"#));
        assert!(content.contains("const count = _useModel<number>(__props, 'count')"));
        assert_eq!(compiled["bindings"]["modelValue"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["any"], json!("setup-ref"));

        let prod = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const modelValue = defineModel<boolean>()\n",
                    "const fn = defineModel<() => void>('fn')\n",
                    "const fnWithDefault = defineModel<() => void>('fnWithDefault', { default: () => null })\n",
                    "const str = defineModel<string>('str')",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "isProd": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = prod["content"].as_str().unwrap_or_default();
        assert!(prod["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("\"modelValue\": { type: Boolean },"));
        assert!(content.contains("\"fn\": {},"));
        assert!(
            content.contains("\"fnWithDefault\": { type: Function, ...{ default: () => null } },")
        );
        assert!(content.contains("\"str\": {},"));
        assert_eq!(prod["bindings"]["modelValue"], json!("setup-ref"));
        assert_eq!(prod["bindings"]["fn"], json!("setup-ref"));
        assert_eq!(prod["bindings"]["fnWithDefault"], json!("setup-ref"));
        assert_eq!(prod["bindings"]["str"], json!("setup-ref"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_type_resolution_deps() {
        let dir =
            std::env::temp_dir().join(format!("vuec-node-bridge-deps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("props.ts"), "export type Props = { foo: string }")
            .expect("write props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './props'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("props.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_resolve_type_projects_props_calls_and_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-resolve-type-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("props.ts"),
            "export type Props = { foo: number; bar?: string; (e: 'save'): void }",
        )
        .expect("write props");

        let filename = dir.join("Comp.vue");
        let resolved = dispatch(
            "sfc.resolveType",
            json!({
                "code": "import type { Props } from './props'\ndefineProps<Props>()",
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 resolveType");

        let expected_dep = dir.join("props.ts").to_string_lossy().replace('\\', "/");
        assert!(resolved["errors"].as_array().unwrap().is_empty());
        assert_eq!(resolved["props"]["foo"], json!(["Number"]));
        assert_eq!(resolved["props"]["bar"], json!(["String"]));
        assert_eq!(resolved["raw"]["props"]["bar"]["optional"], json!(true));
        assert_eq!(resolved["calls"].as_array().unwrap().len(), 1);
        assert_eq!(resolved["deps"], json!([expected_dep]));

        let failed = dispatch(
            "sfc.resolveType",
            json!({
                "code": "defineProps<Missing>()",
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 resolveType failed projection");
        assert!(failed["errors"].as_array().unwrap().iter().any(|error| {
            error
                .as_str()
                .is_some_and(|error| error.contains("Unresolvable type reference"))
        }));

        let _ = std::fs::remove_dir_all(&dir);
    }
