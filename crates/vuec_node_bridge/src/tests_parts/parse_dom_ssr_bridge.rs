    #[test]
    fn vue3_sfc_bridge_compile_script_splits_define_model_transformers() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const modelValue = defineModel({\n",
                    "  get(v) { return v - 1 },\n",
                    "  set: (v) => { return v + 1 },\n",
                    "  required: true\n",
                    "})\n",
                    "const count = defineModel<number>('count', {\n",
                    "  get(v) { return v },\n",
                    "  required: true,\n",
                    "})",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(compact.contains("\"modelValue\": { required: true },"));
        assert!(compact.contains("\"count\": { type: Number, ...{ required: true, } },"));
        assert!(compact.contains("const modelValue = _useModel(__props, \"modelValue\", { get(v) { return v - 1 }, set: (v) => { return v + 1 }, })"));
        assert!(compact.contains(
            "const count = _useModel<number>(__props, 'count', { get(v) { return v }, })"
        ));
        assert_eq!(compiled["bindings"]["modelValue"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
    }

    #[test]
    fn vue3_sfc_bridge_parse_projects_public_descriptor_shape() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template><div>{{ msg }}</div></template>",
                    "<script setup lang=\"ts\">const msg: string = 'hi'</script>",
                    "<style scoped>.a{color:v-bind(color)}</style>",
                    "<i18n lang=\"json\">{\"en\":\"hi\"}</i18n>"
                ),
                "filename": "Comp.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["type"], json!("template"));
        assert_eq!(
            descriptor["template"]["loc"]["source"],
            json!("<div>{{ msg }}</div>")
        );
        assert_eq!(
            descriptor["template"]["ast"]["source"],
            descriptor["source"]
        );
        assert_eq!(
            descriptor["template"]["ast"]["children"][0]["tag"],
            json!("div")
        );
        assert_eq!(descriptor["scriptSetup"]["setup"], json!(true));
        assert_eq!(descriptor["scriptSetup"]["lang"], json!("ts"));
        assert_eq!(descriptor["styles"][0]["scoped"], json!(true));
        assert_eq!(descriptor["cssVars"], json!(["color"]));
        assert_eq!(descriptor["customBlocks"][0]["type"], json!("i18n"));
        assert!(descriptor.get("script_setup").is_none());
        assert_eq!(parsed["errors"], json!([]));
    }

    #[test]
    fn vue3_sfc_bridge_parse_projects_plain_template_lang_as_text_ast() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template lang=\"pug\">p(v-if=\"1 < 2\") test <div/></template>",
                "filename": "Pug.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let ast = &parsed["descriptor"]["template"]["ast"];
        assert_eq!(parsed["errors"], json!([]));
        assert_eq!(ast["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            ast["children"][0]["content"],
            json!("p(v-if=\"1 < 2\") test <div/>")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_uses_dom_void_tags_and_template_options() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template><input><hello/></template><foo> <-& </foo>",
                "filename": "TemplateOptions.vue",
                "options": {
                    "sourceMap": false,
                    "templateParseOptions": {
                        "__vuecCustomElements": ["hello"]
                    }
                }
            }),
        )
        .expect("vue3 sfc parse");

        let template_children = parsed["descriptor"]["template"]["ast"]["children"]
            .as_array()
            .unwrap();
        assert_eq!(parsed["errors"], json!([]));
        assert_eq!(template_children[0]["tag"], json!("input"));
        assert_eq!(template_children[1]["tag"], json!("hello"));
        assert_eq!(template_children[1]["tagType"], json!(0));
        assert_eq!(
            parsed["descriptor"]["customBlocks"][0]["content"],
            json!(" <-& ")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_returns_descriptor_validation_errors() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template>a</template>",
                    "<template>b</template>",
                    "<script src=\"x\"></script>",
                    "<script setup>ok</script>"
                ),
                "filename": "Dup.vue"
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["content"], json!("a"));
        assert!(descriptor["script"].is_null());
        assert_eq!(descriptor["scriptSetup"]["content"], json!("ok"));
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("Single file component can contain only one <template> element")
        );
        assert_eq!(
            parsed["errors"][0]["loc"]["source"],
            json!("<template>b</template>")
        );
        assert_eq!(
            parsed["errors"][1]["message"],
            json!("<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together.")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_preserves_src_presence_and_functional_template_error() {
        let src_parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template src></template><script src></script><style src></style>",
                "filename": "BoolSrc.vue"
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &src_parsed["descriptor"];
        assert_eq!(descriptor["template"]["attrs"]["src"], json!(true));
        assert!(descriptor["template"].get("src").is_none());
        assert!(descriptor["template"].get("map").is_none());
        assert!(descriptor["template"].get("ast").is_none());
        assert_eq!(descriptor["script"]["attrs"]["src"], json!(true));
        assert_eq!(descriptor["styles"][0]["attrs"]["src"], json!(true));
        assert_eq!(src_parsed["errors"], json!([]));

        let functional = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template functional="x"><div/></template>"#,
                "filename": "Functional.vue"
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(
            functional["errors"][0]["message"],
            json!("<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.")
        );
        assert_eq!(
            functional["errors"][0]["loc"]["source"],
            json!("functional=\"x\"")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_decodes_attrs_and_duplicate_attr_errors() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template a="1" a="&amp;">x</template><style module="m&amp;n" setup>.a{}</style>"#,
                "filename": "Attrs.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["attrs"]["a"], json!("&"));
        assert_eq!(descriptor["styles"][0]["module"], json!("m&n"));
        assert!(descriptor["styles"][0].get("setup").is_none());
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("Duplicate attribute.")
        );
        assert_eq!(parsed["errors"][0]["loc"]["start"]["offset"], json!(16));
    }

    #[test]
    fn vue3_sfc_bridge_parse_reports_syntax_errors_from_descriptor_scan() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<?xml?><template><?x?><div/></template><docs><?keep?></docs>"#,
                "filename": "Syntax.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        assert_eq!(
            parsed["descriptor"]["template"]["content"],
            json!("<?x?><div/>")
        );
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("'<?' is allowed only in XML context.")
        );
        assert_eq!(parsed["errors"][0]["loc"]["start"]["offset"], json!(1));
        assert_eq!(parsed["errors"][1]["loc"]["start"]["offset"], json!(18));

        let unclosed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template><div><span>",
                "filename": "Unclosed.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(unclosed["descriptor"]["template"]["content"], json!(""));
        assert_eq!(
            unclosed["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["loc"]["start"]["offset"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![15, 10, 0]
        );

        let malformed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template><div id id></div></template><script>const s = "</script>";</script>"#,
                "filename": "Malformed.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(
            malformed["descriptor"]["script"]["content"],
            json!("const s = \"")
        );
        assert_eq!(
            malformed["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["message"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Duplicate attribute.", "Invalid end tag."]
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_applies_padding_and_ignore_empty_options() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template lang=\"pug\">\n  div\n</template>\n",
                    "<script>\nconst a = 1\n</script>\n",
                    "<style> </style>"
                ),
                "filename": "Pad.vue",
                "options": {
                    "pad": "line",
                    "ignoreEmpty": false,
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["content"], json!("\ndiv\n"));
        assert_eq!(
            descriptor["script"]["content"],
            json!("//\n//\n//\n\nconst a = 1\n")
        );
        assert_eq!(descriptor["styles"][0]["content"], json!("\n\n\n\n\n\n "));
    }

    #[test]
    fn vue27_bridge_compile_script_passes_css_var_options() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>const a = 1</script><style>div{ color: v-bind(color); }</style>",
                "filename": "test.vue",
                "options": {
                    "id": "xxxxxxxx",
                    "isProd": true
                }
            }),
        )
        .expect("vue27 script");

        let content = compiled["content"].as_str().unwrap_or("");
        assert!(content.contains("\"4003f1a6\": (_vm.color)"));
        assert!(content.contains("export default __default__"));
    }

    #[test]
    fn vue27_bridge_compile_script_preserves_script_ast_and_internal_binding_flag() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>export default { props: ['foo'] }</script>",
                "filename": "test.vue",
                "options": {}
            }),
        )
        .expect("vue27 script");

        let script_ast = compiled["scriptAst"].as_array().expect("scriptAst array");
        assert_eq!(script_ast.len(), 1);
        assert_eq!(script_ast[0]["type"], json!("ExportDefaultDeclaration"));
        assert_eq!(
            script_ast[0]["source"],
            json!("export default { props: ['foo'] }")
        );
        assert_eq!(script_ast[0]["loc"]["start"]["offset"], json!(0));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["__isScriptSetup"], json!("false"));
    }

    #[test]
    fn vue27_bridge_compile_script_honors_internal_script_ast_mode() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>export default { props: ['foo'] }</script>",
                "filename": "test.vue",
                "options": {
                    "__vuecScriptAstMode": "none"
                }
            }),
        )
        .expect("vue27 script");

        assert!(compiled.get("scriptAst").is_none());
        assert!(compiled.get("scriptSetupAst").is_none());
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
    }

    #[test]
    fn vue3_dom_bridge_uses_dom_builtin_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({ "source": "<transition/><transition-group/>", "options": {} }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["tagType"], json!(1));
        assert_eq!(parsed["children"][1]["tagType"], json!(1));

        let compiled = dispatch(
            "vue3.dom.compile",
            json!({ "source": "<transition><div/><div/></transition>", "options": {} }),
        )
        .expect("dom compile");

        assert!(compiled["code"]
            .as_str()
            .unwrap_or("")
            .contains("_Transition"));
        assert!(compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(
                |diagnostic| diagnostic.get("message").and_then(Value::as_str)
                    == Some("<Transition> expects exactly one child element or component.")
            ));
    }

    #[test]
    fn vue3_dom_bridge_projects_compile_diagnostic_objects() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<div :bar="a[" v-model="baz"/>"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("dom compile");

        let diagnostics = compiled["diagnostics"].as_array().expect("diagnostics");
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0]["code"], json!(46));
        assert!(diagnostics[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Error parsing JavaScript expression: Unexpected token"));
        assert_eq!(diagnostics[0]["loc"]["start"]["offset"], json!(13));
        assert_eq!(diagnostics[1]["code"], json!(58));
        assert_eq!(diagnostics[1]["loc"]["source"], json!("v-model=\"baz\""));
    }

    #[test]
    fn vue3_dom_bridge_projects_template_expression_public_ast() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": concat!(
                    r#"<FooBar #[foo.slotName] :class="[cond ? '' : bar(), 'default']">"#,
                    r#"{{ `${VAR}VAR2${VAR3}` }}{{ Foo.Bar.Baz }}"#,
                    r#"</FooBar>"#
                ),
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("dom parse");

        let node = &parsed["children"][0];
        let dynamic_arg = &node["props"][0]["arg"]["ast"];
        assert_eq!(dynamic_arg["type"], json!("MemberExpression"));
        assert_eq!(dynamic_arg["object"]["name"], json!("foo"));
        assert_eq!(dynamic_arg["property"]["name"], json!("slotName"));

        let class_exp = &node["props"][1]["exp"]["ast"];
        assert_eq!(class_exp["type"], json!("ArrayExpression"));
        assert_eq!(
            class_exp["elements"][0]["type"],
            json!("ConditionalExpression")
        );
        assert_eq!(class_exp["elements"][0]["test"]["name"], json!("cond"));
        assert_eq!(
            class_exp["elements"][0]["alternate"]["callee"]["name"],
            json!("bar")
        );

        let template_literal = &node["children"][0]["content"]["ast"];
        assert_eq!(template_literal["type"], json!("TemplateLiteral"));
        assert_eq!(template_literal["expressions"][0]["name"], json!("VAR"));
        assert_eq!(template_literal["expressions"][1]["name"], json!("VAR3"));

        let member = &node["children"][1]["content"]["ast"];
        assert_eq!(member["type"], json!("MemberExpression"));
        assert_eq!(member["object"]["object"]["name"], json!("Foo"));
        assert_eq!(member["object"]["property"]["name"], json!("Bar"));
        assert_eq!(member["property"]["name"], json!("Baz"));
    }

    #[test]
    fn vue3_dom_bridge_compile_ast_slices_sfc_template_children() {
        let source =
            "<template><div>{{ msg }}</div></template><script>boom()</script><style>.x{}</style>";
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": source,
                "ast": {
                    "type": 0,
                    "source": source,
                    "children": [{
                        "type": 1,
                        "tag": "div",
                        "loc": {
                            "start": { "offset": 10 },
                            "end": { "offset": 30 },
                            "source": "<div>{{ msg }}</div>"
                        }
                    }]
                },
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true,
                    "__vuecSourceMapSource": source,
                    "__vuecSourceMapBaseOffset": 0
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_ctx.msg"));
        assert!(!compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|diagnostic| diagnostic
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("side effect")));
        assert_eq!(compiled["map"]["sourcesContent"][0], source);
        assert!(compiled["map"]["mappings"].as_str().unwrap_or("").len() > 4);
    }

    #[test]
    fn vue3_ssr_bridge_compile_ast_slices_sfc_template_children() {
        let source =
            "<template><div>{{ msg }}</div></template><script>boom()</script><style>.x{}</style>";
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": source,
                "ast": {
                    "type": 0,
                    "source": source,
                    "children": [{
                        "type": 1,
                        "tag": "div",
                        "loc": {
                            "start": { "offset": 10 },
                            "end": { "offset": 30 },
                            "source": "<div>{{ msg }}</div>"
                        }
                    }]
                },
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true,
                    "__vuecSourceMapSource": source,
                    "__vuecSourceMapBaseOffset": 0
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(!code.contains("boom"));
        assert_eq!(compiled["map"]["sources"], json!(["anonymous.vue"]));
        assert_eq!(compiled["map"]["sourcesContent"][0], source);
        assert!(compiled["map"]["mappings"].as_str().unwrap_or("").len() > 4);
    }

    #[test]
    fn vue3_ssr_bridge_uses_public_compile_defaults() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": "<div>{{ msg }}</div>",
                "options": {
                    "prefixIdentifiers": false,
                    "cacheHandlers": true,
                    "hoistStatic": true,
                    "scopeId": "data-v-x"
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(!code.contains("with (_ctx)"));
        assert!(code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(code.contains("data-v-x"));
        assert!(!code.contains("_hoisted_"));
        assert!(!code.contains("_cache["));
    }

    #[test]
    fn vue3_ssr_bridge_ignores_scope_id_for_explicit_function_mode() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": "<div class=\"a\"></div>",
                "options": {
                    "mode": "function",
                    "scopeId": "data-v-ignored"
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(!code.contains("data-v-ignored"));
        assert!(code.contains("_ssrRenderAttrs(_mergeProps("));
    }

    #[test]
    fn vue3_dom_bridge_uses_dom_namespace_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({ "source": "<svg><rect/></svg><math><ms>1</ms></math>", "options": {} }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["ns"], json!(1));
        assert_eq!(parsed["children"][0]["children"][0]["ns"], json!(1));
        assert_eq!(parsed["children"][1]["ns"], json!(2));
        assert_eq!(parsed["children"][1]["children"][0]["ns"], json!(2));
    }

    #[test]
    fn vue3_dom_bridge_sfc_inner_loc_ends_at_closing_tag_start() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template>\n<div></div>\n</template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let template = &parsed["children"][0];
        assert_eq!(template["innerLoc"]["source"], json!("\n<div></div>\n"));
        assert_eq!(template["innerLoc"]["start"]["offset"], json!(10));
        assert_eq!(template["innerLoc"]["end"]["offset"], json!(23));
    }

    #[test]
    fn vue3_dom_bridge_sfc_inner_loc_offsets_are_utf16() {
        let source = r#"<script>import { "😏" as foo } from './foo'</script><script setup>import { "😏" as foo } from './foo'</script>"#;
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": source,
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let script = &parsed["children"][0];
        let script_setup = &parsed["children"][1];
        assert_eq!(
            script["innerLoc"]["source"],
            json!(r#"import { "😏" as foo } from './foo'"#)
        );
        assert_eq!(script["innerLoc"]["start"]["offset"], json!(8));
        assert_eq!(script["innerLoc"]["end"]["offset"], json!(43));
        assert_eq!(
            script_setup["innerLoc"]["source"],
            json!(r#"import { "😏" as foo } from './foo'"#)
        );
        assert_eq!(script_setup["innerLoc"]["start"]["offset"], json!(66));
        assert_eq!(script_setup["innerLoc"]["end"]["offset"], json!(101));
    }

    #[test]
    fn vue3_core_bridge_projects_public_utils() {
        let position = dispatch(
            "vue3.core.advancePositionWithClone",
            json!({
                "pos": { "offset": 0, "line": 1, "column": 1 },
                "source": "foo\nbar",
                "numberOfCharacters": 4,
            }),
        )
        .expect("position projection");
        assert_eq!(position, json!({ "offset": 4, "line": 2, "column": 1 }));

        let asset = dispatch(
            "vue3.core.toValidAssetId",
            json!({
                "name": "test-测试-1",
                "type": "component",
            }),
        )
        .expect("asset id projection");
        assert_eq!(asset["id"], json!("_component_test_2797935797_1"));
    }

    #[test]
    fn vue3_sfc_bridge_projects_template_utils_url_predicates() {
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isRelativeUrl",
                json!({ "url": "./logo.png" })
            )
            .expect("relative url"),
            json!(true)
        );
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isExternalUrl",
                json!({ "url": "https://vuejs.org/" })
            )
            .expect("external url"),
            json!(true)
        );
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isDataUrl",
                json!({ "url": "data:image/png;base64,i" })
            )
            .expect("data url"),
            json!(true)
        );
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isRelativeUrl",
                json!({ "url": "/logo.png" })
            )
            .expect("absolute url"),
            json!(false)
        );
    }

    #[test]
    fn vue3_dom_bridge_sfc_plain_template_lang_keeps_raw_text() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template lang=\"pug\">p(v-if=\"1 < 2\") test <div/></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let template = &parsed["children"][0];
        assert_eq!(template["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            template["children"][0]["content"],
            json!("p(v-if=\"1 < 2\") test <div/>")
        );
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_parse_uses_dom_void_tag_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><input></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let input = &parsed["children"][0]["children"][0];
        assert_eq!(input["tag"], json!("input"));
        assert_eq!(input["children"].as_array().unwrap().len(), 0);
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_custom_blocks_are_raw_text() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><input></template><foo> <-& </foo>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let custom_block = &parsed["children"][1];
        assert_eq!(custom_block["tag"], json!("foo"));
        assert_eq!(custom_block["children"].as_array().unwrap().len(), 1);
        assert_eq!(custom_block["children"][0]["content"], json!(" <-& "));
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_parse_classifies_non_native_tags_as_components() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><hello/></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let hello = &parsed["children"][0]["children"][0];
        assert_eq!(hello["tag"], json!("hello"));
        assert_eq!(hello["tagType"], json!(1));

        let custom = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><hello/></template>",
                "options": {
                    "parseMode": "sfc",
                    "__vuecCustomElements": ["hello"]
                }
            }),
        )
        .expect("dom parse");
        assert_eq!(custom["children"][0]["children"][0]["tagType"], json!(0));
    }

    #[test]
    fn vue3_dom_bridge_allows_v_model_on_custom_elements() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<my-input v-model="value"/>"#,
                "options": {
                    "__vuecCustomElements": ["my-input"]
                }
            }),
        )
        .expect("dom compile");

        assert!(compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty());
        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("vModelText"));
        assert!(code.contains("_withDirectives"));
    }

    #[test]
    fn vue3_dom_bridge_respects_explicit_empty_dom_parser_predicates() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<input><hello/>",
                "options": {
                    "__vuecVoidTags": [],
                    "__vuecNativeTags": []
                }
            }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["children"][0]["tag"], json!("hello"));
        assert!(parsed["__vuecDiagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == json!(24)));
    }

    #[test]
    fn vue3_dom_bridge_parses_asset_url_options() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png"><img src="~bar.png">"#,
                "options": {
                    "transformAssetUrls": {
                        "base": "/foo"
                    }
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#"src: "/foo/bar.png""#));
        assert!(code.contains(r#"src: "~bar.png""#));

        let disabled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "transformAssetUrls": false
                }
            }),
        )
        .expect("dom compile");

        assert!(disabled["code"]
            .as_str()
            .unwrap_or("")
            .contains(r#"src: "./bar.png""#));
    }

    #[test]
    fn vue27_bridge_compile_template_transforms_asset_urls() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><img src="./logo.png" srcset="./logo.png 2x"><svg><use href="~@svg/file.svg#fragment"/></svg></div>"#,
                "options": {
                    "transformAssetUrls": {
                        "use": "href"
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#""src":require("./logo.png")"#));
        assert!(code.contains(r#""srcset":require("./logo.png") + " 2x""#));
        assert!(code.contains(r##""href":require("@svg/file.svg") + "#fragment""##));
    }

    #[test]
    fn vue27_bridge_compile_template_asset_options_support_base_and_absolute_urls() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><img src="./logo.png"><img src="/logo.png"><img src="@/logo.png"></div>"#,
                "options": {
                    "transformAssetUrls": true,
                    "transformAssetUrlsOptions": {
                        "base": "/base/",
                        "includeAbsolute": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#""src":"/base/logo.png""#));
        assert!(code.contains(r#""src":require("/logo.png")"#));
        assert!(code.contains(r#""src":require("@/logo.png")"#));
    }

    #[test]
    fn vue27_bridge_compile_template_preprocesses_pug_and_reports_missing_lang() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "body\n h1 Pug Examples\n div.container\n   p Cool Pug example!\n",
                "filename": "example.vue",
                "options": {
                    "preprocessLang": "pug"
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert!(compiled["errors"].as_array().unwrap().is_empty());
        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_c('body'"));
        assert!(code.contains("staticClass:\"container\""));

        let missing = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "",
                "filename": "example.vue",
                "options": {
                    "preprocessLang": "unknownLang"
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(missing["errors"].as_array().unwrap().len(), 1);
        assert_eq!(missing["tips"].as_array().unwrap().len(), 1);
        assert!(missing["errors"][0]
            .as_str()
            .unwrap_or("")
            .contains("unknownLang"));
        assert_eq!(
            missing["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
    }

    #[test]
    fn vue27_bridge_compile_template_returns_empty_render_on_vue2_errors() {
        let plain = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "<div></div><span></span><p></p>",
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": false
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(
            plain["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
        assert_eq!(plain["errors"].as_array().unwrap().len(), 1);
        assert_eq!(
            plain["errors"][0],
            json!("Component template should contain exactly one root element. If you are using v-if on multiple elements, use v-else-if to chain them instead.")
        );

        let ranged = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "<div></div><span></span><p></p>",
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(
            ranged["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
        assert_eq!(ranged["errors"].as_array().unwrap().len(), 1);
        assert_eq!(ranged["errors"][0]["start"], json!(11));
        assert!(ranged["errors"][0].get("end").is_none());
    }

    #[test]
    fn vue27_bridge_compile_template_projects_vue2_tip_ranges_from_compiler_options() {
        let plain = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><el-dropdown-item v-for="item in handle">{{ item.label }}</el-dropdown-item></div>"#,
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": false
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(plain["tips"].as_array().unwrap().len(), 1);
        assert_eq!(
            plain["tips"][0],
            json!(
                r#"<el-dropdown-item v-for="item in handle">: component lists rendered with v-for should have explicit keys. See https://v2.vuejs.org/v2/guide/list.html#key for more info."#
            )
        );

        let ranged = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><el-dropdown-item v-for="item in handle">{{ item.label }}</el-dropdown-item></div>"#,
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(ranged["tips"].as_array().unwrap().len(), 1);
        assert_eq!(
            ranged["tips"][0]["msg"],
            json!(
                r#"<el-dropdown-item v-for="item in handle">: component lists rendered with v-for should have explicit keys. See https://v2.vuejs.org/v2/guide/list.html#key for more info."#
            )
        );
        assert_eq!(ranged["tips"][0]["start"], json!(23));
        assert_eq!(ranged["tips"][0]["end"], json!(45));
        assert!(ranged["tips"][0].get("tip").is_none());

        let leading = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "\n<div><el-dropdown-item v-for=\"item in handle\">{{ item.label }}</el-dropdown-item></div>\n",
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(leading["tips"].as_array().unwrap().len(), 1);
        assert_eq!(leading["tips"][0]["start"], json!(24));
        assert_eq!(leading["tips"][0]["end"], json!(46));
        assert!(leading["tips"][0].get("tip").is_none());
    }

    #[test]
    fn vue3_dom_bridge_projects_asset_url_imports() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png" srcset="./bar.png 2x">"#,
                "options": {
                    "mode": "module"
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!code.contains("_ctx._imports_"));

        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "mode": "module"
                }
            }),
        )
        .expect("dom parse");

        assert_eq!(parsed["imports"], json!([]));
    }

    #[test]
    fn vue3_sfc_compile_template_uses_bridge_options_for_hoist_static() {
        let compiled = dispatch(
            "sfc.compileTemplate",
            json!({
                "source": r#"<div><img src="./bar.png"><span>ok</span></div>"#,
                "filename": "template.vue",
                "options": {
                    "compilerOptions": {
                        "hoistStatic": false
                    }
                },
                "bridgeOptions": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "cacheHandlers": true,
                    "sourceMap": true,
                    "hoistStatic": false
                }
            }),
        )
        .expect("sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert_eq!(compiled["map"]["version"], json!(3));
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(!code.contains("_cache[0]"));

        let without_map = dispatch(
            "sfc.compileTemplate",
            json!({
                "source": "<div>{{ msg }}</div>",
                "filename": "template.vue",
                "bridgeOptions": {
                    "sourceMap": false
                }
            }),
        )
        .expect("sfc compileTemplate without source map");
        assert!(without_map["map"].is_null());
    }

    #[test]
    fn vue3_dom_bridge_stringifies_static_children_from_sentinel_option() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                "options": {
                    "prefixIdentifiers": true,
                    "hoistStatic": true,
                    "__vuecStringifyStatic": true
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("createStaticVNode"));
        assert!(code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn vue3_ssr_bridge_projects_asset_url_imports() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<img src="./bar.png" srcset="./bar.png 2x">"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(code.contains("_attrs"));
        assert!(!code.contains("_ctx._imports_"));

        let disabled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "transformAssetUrls": false
                }
            }),
        )
        .expect("ssr compile");

        let disabled_code = disabled["code"].as_str().unwrap_or("");
        assert!(!disabled_code.contains("import _imports_0"));
        assert!(disabled_code.contains(r#"src: "./bar.png""#));
        assert!(disabled_code.contains("_attrs"));
    }

    #[test]
    fn vue3_ssr_bridge_uses_dom_parser_defaults_for_components() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<router-link><img src="./logo.png"></router-link>"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("resolveComponent as _resolveComponent"));
        assert!(code.contains("const _component_router_link = _resolveComponent(\"router-link\")"));
        assert!(code.contains("_push(_ssrRenderComponent(_component_router_link, _attrs, {"));
        assert!(code.contains("_createVNode(\"img\", { src: _imports_0 })"));
    }
