    use crate::*;
    use vuec_js::JsStringInternerStats;

    fn compact_js_whitespace(source: &str) -> String {
        source.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn assert_script_import_binding(
        script: &SfcScriptBlock,
        local: &str,
        imported: &str,
        source: &str,
        is_type: bool,
        is_from_setup: bool,
        is_used_in_template: bool,
    ) {
        let binding = script
            .imports
            .get(local)
            .unwrap_or_else(|| panic!("missing import binding for {local}"));
        assert_eq!(binding.imported, imported);
        assert_eq!(binding.local, local);
        assert_eq!(binding.source, source);
        assert_eq!(binding.is_type, is_type);
        assert_eq!(binding.is_from_setup, is_from_setup);
        assert_eq!(binding.is_used_in_template, is_used_in_template);
    }

    #[test]
    fn template_usage_index_applies_vue27_and_vue3_ts_rules() {
        let vue27 = TemplateUsageIndex::new(
            r#"{{ `${VAR}VAR2${VAR3}` }}"#,
            TemplateUsageFlavor::Vue27,
            true,
        );
        assert!(vue27.contains("VAR"));
        assert!(vue27.contains("VAR3"));
        assert!(!vue27.contains("VAR2"));

        let vue3 = TemplateUsageIndex::new(
            r#"<FooBar #[foo.slotName] />
<div :[bar.attrName]="15"></div>
<div>{{ a as Foo }}</div>
<div>{{ Baz }}</div>
<FooBar :msg />"#,
            TemplateUsageFlavor::Vue3,
            true,
        );
        assert!(vue3.contains("FooBar"));
        assert!(vue3.contains("foo"));
        assert!(vue3.contains("bar"));
        assert!(vue3.contains("Baz"));
        assert!(vue3.contains("msg"));
        assert!(!vue3.contains("Foo"));
    }

    #[test]
    fn template_v_model_identifiers_stream_all_start_tags() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Component.vue",
            r#"<template>
                <input v-model="first">
                <input v-model.trim="second">
                <input v-model="nested.value">
                <input v-model="undefined">
                <input v-model>
            </template>"#,
        );

        assert_eq!(
            vue3_template_v_model_identifiers(&descriptor),
            BTreeSet::from(["first".to_string(), "second".to_string()])
        );
    }

    fn generated_original_position(
        script: &SfcScriptBlock,
        generated_needle: &str,
    ) -> vuec_codegen::SourceMapOriginalPosition {
        let map = script.map.as_ref().expect("script source map");
        let offset = script
            .content
            .find(generated_needle)
            .unwrap_or_else(|| panic!("generated needle not found: {generated_needle}"));
        let (line, column) = utf16_zero_based_line_column_for_byte_offset(&script.content, offset)
            .expect("generated position");
        map.original_position(vuec_source::GeneratedPosition::new(
            line as u32,
            column as u32,
        ))
        .expect("source map lookup")
        .unwrap_or_else(|| panic!("missing original mapping for {generated_needle}"))
    }

    fn original_line_column(source: &str, source_needle: &str) -> (u32, u32) {
        let offset = source
            .find(source_needle)
            .unwrap_or_else(|| panic!("source needle not found: {source_needle}"));
        let (line, column) =
            utf16_zero_based_line_column_for_byte_offset(source, offset).expect("source position");
        (line as u32, column as u32)
    }

    #[test]
    fn parses_blocks() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script setup lang="ts">const x = 1</script><style scoped>.a{}</style>"#,
        );
        assert!(descriptor.template.is_some());
        assert!(descriptor.script_setup.is_some());
        assert_eq!(descriptor.styles.len(), 1);
    }

    #[test]
    fn vue3_public_parse_projection_uses_official_descriptor_keys() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            concat!(
                r#"<template><div>{{ msg }}</div></template>"#,
                r#"<script setup lang="ts">const msg: string = 'hi'</script>"#,
                r#"<style scoped module>.a{ color: v-bind(color); }</style>"#,
                r#"<i18n lang="json">{"en":"hi"}</i18n>"#,
            ),
        );
        let projected = vue3_sfc_descriptor_value(
            &descriptor,
            &Vue3SfcParseProjectionOptions {
                source_map: false,
                source_root: String::new(),
                pad: Vue3SfcPad::False,
            },
        );

        assert_eq!(projected["scriptSetup"]["type"], json!("script"));
        assert_eq!(projected["scriptSetup"]["setup"], json!(true));
        assert_eq!(projected["scriptSetup"]["lang"], json!("ts"));
        assert!(projected.get("script_setup").is_none());
        assert_eq!(projected["styles"][0]["attrs"]["scoped"], json!(true));
        assert_eq!(projected["styles"][0]["module"], json!(true));
        assert_eq!(projected["customBlocks"][0]["type"], json!("i18n"));
        assert_eq!(projected["customBlocks"][0]["lang"], json!("json"));
        assert_eq!(projected["cssVars"], json!(["color"]));
        assert_eq!(
            projected["template"]["loc"]["source"],
            json!("<div>{{ msg }}</div>")
        );
        assert!(projected["template"].get("map").is_none());
    }

    #[test]
    fn vue3_public_parse_projection_maps_empty_attr_values_like_vue3() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<template src=""></template><script setup="named">x</script><style scoped="x" module="" src=""></style>"#,
        );
        let projected = vue3_sfc_descriptor_value(
            &descriptor,
            &Vue3SfcParseProjectionOptions {
                source_map: true,
                source_root: String::new(),
                pad: Vue3SfcPad::False,
            },
        );

        assert_eq!(projected["template"]["attrs"]["src"], json!(true));
        assert_eq!(projected["template"]["src"], json!(""));
        assert_eq!(projected["scriptSetup"]["attrs"]["setup"], json!("named"));
        assert_eq!(projected["scriptSetup"]["setup"], json!("named"));
        assert_eq!(projected["styles"][0]["attrs"]["module"], json!(true));
        assert_eq!(projected["styles"][0]["module"], json!(true));
        assert_eq!(projected["styles"][0]["attrs"]["scoped"], json!("x"));
        assert_eq!(projected["styles"][0]["scoped"], json!(true));
        assert!(projected["styles"][0].get("map").is_none());
    }

    #[test]
    fn vue3_parse_decodes_attrs_and_reports_duplicate_attrs_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Attrs.vue",
            r#"<template a="1" a="&amp;" lang="p&amp;g">x</template><style module="m&amp;n" setup>.a{}</style><script setup generic="T &amp; U">y</script>"#,
        );

        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["descriptor"]["template"]["attrs"]["a"],
            json!("&")
        );
        assert_eq!(
            projected["descriptor"]["template"]["attrs"]["lang"],
            json!("p&g")
        );
        assert_eq!(projected["descriptor"]["template"]["lang"], json!("p&g"));
        assert_eq!(
            projected["descriptor"]["styles"][0]["attrs"]["module"],
            json!("m&n")
        );
        assert_eq!(projected["descriptor"]["styles"][0]["module"], json!("m&n"));
        assert!(projected["descriptor"]["styles"][0].get("setup").is_none());
        assert_eq!(
            projected["descriptor"]["scriptSetup"]["attrs"]["generic"],
            json!("T & U")
        );
        assert_eq!(
            projected["errors"][0]["message"],
            json!("Duplicate attribute.")
        );
        assert_eq!(projected["errors"][0]["loc"]["source"], json!(""));
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(16));
    }

    #[test]
    fn vue3_parse_reports_bogus_question_tags_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Question.vue",
            r#"<?xml?><template><?x?><div/></template><docs><?keep?></docs>"#,
        );

        assert_eq!(
            result.descriptor.template.as_ref().unwrap().content,
            "<?x?><div/>"
        );
        assert_eq!(result.descriptor.custom_blocks[0].content, "<?keep?>");
        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["message"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "'<?' is allowed only in XML context.",
                "'<?' is allowed only in XML context.",
            ]
        );
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(1));
        assert_eq!(projected["errors"][1]["loc"]["start"]["offset"], json!(18));
    }

    #[test]
    fn vue3_parse_reports_missing_end_tags_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let script = compiler.parse_vue3("UnclosedScript.vue", "<script>x");
        assert_eq!(script.descriptor.script.as_ref().unwrap().content, "");
        assert_eq!(script.errors[0].message, "Element is missing end tag.");
        assert_eq!(script.errors[0].loc.as_ref().unwrap().start, 0);

        let nested = compiler.parse_vue3("Nested.vue", "<template><div><span></template>");
        assert_eq!(
            nested.descriptor.template.as_ref().unwrap().content,
            "<div><span>"
        );
        assert_eq!(nested.errors.len(), 1);
        assert_eq!(nested.errors[0].loc.as_ref().unwrap().start, 15);

        let eof = compiler.parse_vue3("Eof.vue", "<template><div><span>");
        assert_eq!(eof.descriptor.template.as_ref().unwrap().content, "");
        assert_eq!(
            eof.errors
                .iter()
                .map(|error| error.loc.as_ref().unwrap().start)
                .collect::<Vec<_>>(),
            vec![15, 10, 0]
        );

        let custom = compiler.parse_vue3("Custom.vue", "<template/><docs><?x?");
        assert_eq!(custom.descriptor.custom_blocks[0].content, "");
        assert_eq!(custom.errors.len(), 1);
        assert_eq!(custom.errors[0].loc.as_ref().unwrap().start, 11);
    }

    #[test]
    fn vue3_parse_uses_dom_void_tags_inside_template_blocks() {
        let mut compiler = SfcCompiler::new();
        let result =
            compiler.parse_vue3("Void.vue", "<template><input></template><foo> <-& </foo>");

        assert!(result.errors.is_empty());
        assert_eq!(
            result.descriptor.template.as_ref().unwrap().content,
            "<input>"
        );
        assert_eq!(result.descriptor.custom_blocks[0].content, " <-& ");
    }

    #[test]
    fn vue3_parse_reports_malformed_descriptor_syntax_like_official_parser() {
        let mut compiler = SfcCompiler::new();

        let uppercase = compiler.parse_vue3("Upper.vue", "<SCRIPT>let a=1</SCRIPT>");
        assert_eq!(uppercase.descriptor.custom_blocks[0].type_name, "SCRIPT");
        assert_eq!(uppercase.descriptor.custom_blocks[0].content, "");
        assert_eq!(uppercase.errors[0].message, "Element is missing end tag.");
        assert_eq!(uppercase.errors[0].loc.as_ref().unwrap().start, 0);

        let raw_extra =
            compiler.parse_vue3("RawExtra.vue", r#"<script>const s = "</script>";</script>"#);
        assert_eq!(
            raw_extra.descriptor.script.as_ref().unwrap().content,
            "const s = \""
        );
        assert_eq!(raw_extra.errors[0].message, "Invalid end tag.");
        assert_eq!(raw_extra.errors[0].loc.as_ref().unwrap().start, 30);

        let cdata = compiler.parse_vue3("Cdata.vue", "<template><![CDATA[x]]></template>");
        assert_eq!(cdata.descriptor.template.as_ref().unwrap().content, "");
        assert_eq!(
            cdata.errors[0].message,
            "CDATA section is allowed only in XML context."
        );
        assert_eq!(cdata.errors[0].loc.as_ref().unwrap().start, 10);

        let invalid_end = compiler.parse_vue3("InvalidEnd.vue", "<template></div></template>");
        assert_eq!(
            invalid_end.descriptor.template.as_ref().unwrap().content,
            ""
        );
        assert_eq!(invalid_end.errors[0].message, "Invalid end tag.");
        assert_eq!(invalid_end.errors[0].loc.as_ref().unwrap().start, 10);

        let invalid_attr = compiler.parse_vue3("InvalidAttr.vue", "<template =x></template>");
        assert_eq!(
            invalid_attr.errors[0].message,
            "Attribute name cannot start with '='."
        );
        assert_eq!(invalid_attr.errors[0].loc.as_ref().unwrap().start, 10);

        let missing_value = compiler.parse_vue3("MissingValue.vue", "<template a=></template>");
        assert_eq!(
            missing_value.errors[0].message,
            "Attribute value was expected."
        );
        assert_eq!(missing_value.errors[0].loc.as_ref().unwrap().start, 12);

        let nested_duplicate = compiler.parse_vue3(
            "NestedDuplicate.vue",
            "<template><div id id></div></template>",
        );
        assert_eq!(nested_duplicate.errors[0].message, "Duplicate attribute.");
        assert_eq!(nested_duplicate.errors[0].loc.as_ref().unwrap().start, 18);
    }

    #[test]
    fn vue3_parse_preserves_boolean_src_attr_presence_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "BoolSrc.vue",
            "<template src></template><script src></script><style src></style>",
        );

        assert!(result.errors.is_empty());
        assert!(result
            .descriptor
            .template
            .as_ref()
            .unwrap()
            .attrs
            .has_src_attr());
        assert!(result
            .descriptor
            .script
            .as_ref()
            .unwrap()
            .attrs
            .has_src_attr());
        assert!(result.descriptor.styles[0].attrs.has_src_attr());

        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["descriptor"]["template"]["attrs"]["src"],
            json!(true)
        );
        assert!(projected["descriptor"]["template"].get("src").is_none());
        assert!(projected["descriptor"]["template"].get("map").is_none());
        assert_eq!(
            projected["descriptor"]["script"]["attrs"]["src"],
            json!(true)
        );
        assert!(projected["descriptor"]["script"].get("src").is_none());
        assert!(projected["descriptor"]["script"].get("map").is_none());
        assert_eq!(
            projected["descriptor"]["styles"][0]["attrs"]["src"],
            json!(true)
        );
        assert!(projected["descriptor"]["styles"][0].get("src").is_none());
        assert!(projected["descriptor"]["styles"][0].get("map").is_none());
    }

    #[test]
    fn vue3_parse_reports_duplicate_blocks_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Dup.vue",
            "<template>a</template><template>b</template><script>one</script><script>two</script><script setup>first</script><script setup>second</script>",
        );

        assert_eq!(result.descriptor.template.as_ref().unwrap().content, "a");
        assert_eq!(result.descriptor.script.as_ref().unwrap().content, "one");
        assert_eq!(
            result.descriptor.script_setup.as_ref().unwrap().content,
            "first"
        );
        assert_eq!(
            result
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Single file component can contain only one <template> element",
                "Single file component can contain only one <script> element",
                "Single file component can contain only one <script setup> element",
            ]
        );
        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["errors"][0]["loc"]["source"],
            json!("<template>b</template>")
        );
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(22));
    }

    #[test]
    fn parsed_vue3_compile_results_preserve_descriptor_errors() {
        let mut compiler = SfcCompiler::new();
        let parsed = compiler.parse_vue3(
            "Duplicate.vue",
            "<template><div/></template><template><span/></template><script>const one = 1</script><script>const two = 2</script><style>.a{}</style>",
        );
        let expected = [
            "Single file component can contain only one <template> element",
            "Single file component can contain only one <script> element",
        ];

        let template = compiler.compile_parsed_vue3_template(
            &parsed,
            SfcTemplateCompileOptions::default(),
        );
        assert_eq!(
            template
                .errors
                .iter()
                .take(2)
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            template.errors[0].loc.source,
            "<template><span/></template>"
        );

        let script = compiler
            .compile_parsed_vue3_script(&parsed, SfcScriptCompileOptions::default());
        assert_eq!(
            script
                .errors
                .iter()
                .take(2)
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );

        let style =
            compiler.compile_parsed_vue3_style(&parsed, SfcStyleCompileOptions::default());
        assert_eq!(
            style
                .errors
                .iter()
                .take(2)
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(style.diagnostics[0].code, "VUEC_SFC_PARSE");
        assert_eq!(style.diagnostics[1].code, "VUEC_SFC_PARSE");

        let unclosed = compiler.parse_vue3("Unclosed.vue", "<script>const value = 1");
        let script = compiler
            .compile_parsed_vue3_script(&unclosed, SfcScriptCompileOptions::default());
        assert_eq!(script.errors[0], "Element is missing end tag.");
    }

    #[test]
    fn vue3_parse_applies_script_src_and_empty_script_rules() {
        let mut compiler = SfcCompiler::new();
        let empty_script =
            compiler.parse_vue3("Empty.vue", "<script>  \n</script><style>x</style>");
        assert!(empty_script.descriptor.script.is_none());
        assert_eq!(
            empty_script.errors[0].message,
            "At least one <template> or <script> is required in a single file component. Empty.vue"
        );

        let setup_src = compiler.parse_vue3(
            "SetupSrc.vue",
            r#"<script setup src="x"></script><script setup>ok</script>"#,
        );
        assert!(setup_src.descriptor.script_setup.is_none());
        assert_eq!(
            setup_src
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Single file component can contain only one <script setup> element",
                "<script setup> cannot use the \"src\" attribute because its syntax will be ambiguous outside of the component.",
            ]
        );

        let script_src_with_setup = compiler.parse_vue3(
            "SrcAndSetup.vue",
            r#"<script src="x"></script><script setup>ok</script>"#,
        );
        assert!(script_src_with_setup.descriptor.script.is_none());
        assert_eq!(
            script_src_with_setup
                .descriptor
                .script_setup
                .as_ref()
                .unwrap()
                .content,
            "ok"
        );
        assert_eq!(
            script_src_with_setup.errors[0].message,
            "<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together."
        );

        let empty_src_with_setup = compiler.parse_vue3(
            "EmptySrcAndSetup.vue",
            r#"<script src=""></script><script setup src=""></script>"#,
        );
        assert!(empty_src_with_setup.errors.is_empty());
        assert!(empty_src_with_setup.descriptor.script.is_some());
        assert!(empty_src_with_setup.descriptor.script_setup.is_some());
    }

    #[test]
    fn vue3_parse_reports_functional_template_attr_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Functional.vue",
            r#"<template functional="x"><div/></template><template functional>b</template>"#,
        );

        assert_eq!(
            result
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.",
                "Single file component can contain only one <template> element",
            ]
        );
        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["errors"][0]["loc"]["source"],
            json!("functional=\"x\"")
        );
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(10));
        assert_eq!(
            projected["errors"][1]["loc"]["source"],
            json!("<template functional>b</template>")
        );
    }

    #[test]
    fn vue3_parse_options_pad_non_template_blocks_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let source = concat!(
            "<template>\n  div\n</template>\n",
            "<script>\nconst a = 1\n</script>\n",
            "<style>\n.a{}\n</style>\n",
            "<i18n>\n{}\n</i18n>"
        );
        let line = compiler.parse_vue3_with_options(
            "Pad.vue",
            source,
            Vue3SfcParseOptions {
                pad: Vue3SfcPad::Line,
                ..Vue3SfcParseOptions::default()
            },
        );

        assert_eq!(
            line.descriptor.template.as_ref().unwrap().content,
            "\n  div\n"
        );
        assert_eq!(
            line.descriptor.script.as_ref().unwrap().content,
            "//\n//\n//\n\nconst a = 1\n"
        );
        assert_eq!(line.descriptor.styles[0].content, "\n\n\n\n\n\n\n.a{}\n");
        assert_eq!(
            line.descriptor.custom_blocks[0].content,
            "\n\n\n\n\n\n\n\n\n\n{}\n"
        );

        let space = compiler.parse_vue3_with_options(
            "Pad.vue",
            source,
            Vue3SfcParseOptions {
                pad: Vue3SfcPad::Space,
                ..Vue3SfcParseOptions::default()
            },
        );
        assert!(space
            .descriptor
            .script
            .as_ref()
            .unwrap()
            .content
            .starts_with("          \n"));
        assert!(space.descriptor.styles[0].content.ends_with(".a{}\n"));
    }

    #[test]
    fn vue3_parse_options_ignore_empty_and_dedent_pug_template() {
        let mut compiler = SfcCompiler::new();
        let source = concat!(
            "<template lang=\"pug\">\n  div\n    span\n</template>",
            "<script> </script><style> </style><i18n> </i18n>"
        );
        let default = compiler.parse_vue3("Pug.vue", source);
        assert_eq!(
            default.descriptor.template.as_ref().unwrap().content,
            "\ndiv\n  span\n"
        );
        assert!(default.descriptor.script.is_none());
        assert!(default.descriptor.styles.is_empty());
        assert!(default.descriptor.custom_blocks.is_empty());

        let keep_empty = compiler.parse_vue3_with_options(
            "Pug.vue",
            source,
            Vue3SfcParseOptions {
                ignore_empty: false,
                ..Vue3SfcParseOptions::default()
            },
        );
        assert_eq!(keep_empty.descriptor.script.as_ref().unwrap().content, " ");
        assert_eq!(keep_empty.descriptor.styles[0].content, " ");
        assert_eq!(keep_empty.descriptor.custom_blocks[0].content, " ");
    }

    #[test]
    fn parse_descriptor_cache_hits_and_invalidates_by_source_hash() {
        let mut compiler = SfcCompiler::new();
        let first = compiler.parse("foo.vue", r#"<template><div>{{ a }}</div></template>"#);
        let second = compiler.parse("foo.vue", r#"<template><div>{{ a }}</div></template>"#);
        assert_eq!(first, second);
        assert_eq!(compiler.descriptor_cache_len(), 1);
        assert_eq!(
            compiler.cache_stats(),
            SfcCacheStats {
                descriptor_hits: 1,
                descriptor_misses: 1,
                descriptor_invalidations: 0,
            }
        );

        let changed = compiler.parse("foo.vue", r#"<template><span>{{ b }}</span></template>"#);
        assert_ne!(
            first.template.as_ref().unwrap().content,
            changed.template.as_ref().unwrap().content
        );
        assert_eq!(compiler.descriptor_cache_len(), 1);
        assert_eq!(
            compiler.cache_stats(),
            SfcCacheStats {
                descriptor_hits: 1,
                descriptor_misses: 2,
                descriptor_invalidations: 1,
            }
        );
    }

    #[test]
    fn clear_caches_releases_descriptor_and_js_lifecycle_state() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "state.vue",
            r#"<script setup>const count = 1</script><template>{{ count }}</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(compiler.descriptor_cache_len(), 1);
        assert!(!script.script_setup_ast.is_empty());
        assert!(compiler.js().string_interner_stats().entries > 0);
        assert_eq!(
            compiler.cache_stats(),
            SfcCacheStats {
                descriptor_hits: 0,
                descriptor_misses: 1,
                descriptor_invalidations: 0,
            }
        );

        compiler.clear_caches();

        assert_eq!(compiler.descriptor_cache_len(), 0);
        assert_eq!(
            compiler.js().string_interner_stats(),
            JsStringInternerStats::default()
        );
        assert_eq!(compiler.cache_stats(), SfcCacheStats::default());

        let reparsed = compiler.parse("state.vue", r#"<template><span>fresh</span></template>"#);
        assert_eq!(compiler.descriptor_cache_len(), 1);
        assert_eq!(
            reparsed.template.as_ref().unwrap().content,
            "<span>fresh</span>"
        );
    }

    #[test]
    fn vue27_parse_cache_preserves_error_projection() {
        let mut compiler = SfcCompiler::new();
        let source = "<template><div></template>";
        let options = Vue27ParseComponentOptions {
            output_source_range: true,
            ..Vue27ParseComponentOptions::default()
        };
        let first =
            compiler.parse_vue27_component_with_filename("bad.vue", source, options.clone());
        let second = compiler.parse_vue27_component_with_filename("bad.vue", source, options);
        assert_eq!(first.errors, second.errors);
        assert!(second.errors.iter().any(|error| error.start.is_some()));

        let masked = compiler.parse_vue27_component_with_filename(
            "bad-masked.vue",
            source,
            Vue27ParseComponentOptions::default(),
        );
        let masked_hit = compiler.parse_vue27_component_with_filename(
            "bad-masked.vue",
            source,
            Vue27ParseComponentOptions::default(),
        );
        assert_eq!(masked.errors, masked_hit.errors);
        assert!(masked_hit
            .errors
            .iter()
            .all(|error| error.start.is_none() && error.end.is_none()));
    }

    #[test]
    fn vue27_parse_component_preserves_top_level_blocks_and_attrs() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue27_component(
            r#"
<template><div><style>nested</style></div></template>
<style bool-attr val-attr="test" module></style>
<example name="simple"><my-button>Hello</my-button></example>
<div><style>ignored</style></div>
"#,
            Vue27ParseComponentOptions::default(),
        );

        let descriptor = result.descriptor;
        assert_eq!(
            descriptor.template.as_ref().unwrap().content.trim(),
            "<div><style>nested</style></div>"
        );
        assert_eq!(descriptor.styles.len(), 1);
        assert_eq!(
            descriptor.styles[0].attrs.raw.get("bool-attr"),
            Some(&SfcAttrValue::Bool(true))
        );
        assert_eq!(
            descriptor.styles[0].attrs.raw.get("val-attr"),
            Some(&SfcAttrValue::String("test".into()))
        );
        assert_eq!(descriptor.styles[0].attrs.module.as_deref(), Some(""));
        assert_eq!(descriptor.custom_blocks.len(), 2);
        assert_eq!(descriptor.custom_blocks[0].type_name, "example");
        assert_eq!(
            descriptor.custom_blocks[0].content.trim(),
            "<my-button>Hello</my-button>"
        );
        assert_eq!(descriptor.custom_blocks[1].type_name, "div");
    }

    #[test]
    fn vue27_parse_component_deindents_like_official_parser() {
        let content = r#"<template>
        <div></div>
      </template>
      <script>
        export default {}
      </script>
      <style>
        h1 { color: red }
      </style>"#;
        let mut compiler = SfcCompiler::new();
        let default = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::False,
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            default.descriptor.template.unwrap().content,
            "\n<div></div>\n"
        );
        assert_eq!(
            default.descriptor.script.unwrap().content,
            "\n        export default {}\n      "
        );
        assert_eq!(
            default.descriptor.styles[0].content,
            "\nh1 { color: red }\n"
        );

        let enabled = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            enabled.descriptor.script.unwrap().content,
            "\nexport default {}\n"
        );
    }

    #[test]
    fn vue27_parse_component_pads_non_template_content() {
        let content = r#"<template>
        <div></div>
      </template>
      <script>
        export default {}
      </script>
      <style>
        h1 { color: red }
      </style>"#;
        let mut compiler = SfcCompiler::new();
        let line = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::Line,
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            line.descriptor.script.unwrap().content,
            format!("{}\nexport default {{}}\n", "//\n".repeat(3))
        );
        assert_eq!(
            line.descriptor.styles[0].content,
            "\n\n\n\n\n\n\nh1 { color: red }\n"
        );

        let space = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::Space,
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        let script_pad = content[..space.descriptor.script.as_ref().unwrap().content_start]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect::<String>();
        assert_eq!(
            space.descriptor.script.unwrap().content,
            script_pad + "\nexport default {}\n"
        );
    }

    #[test]
    fn vue27_parse_component_recovers_unclosed_template_with_source_range() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue27_component(
            "<template>hi</",
            Vue27ParseComponentOptions {
                output_source_range: true,
                ..Vue27ParseComponentOptions::default()
            },
        );

        assert_eq!(result.descriptor.template.unwrap().content, "hi");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].start, Some(0));
        assert_eq!(result.errors[0].end, Some(10));
    }

    #[test]
    fn vue27_rewrite_default_handles_default_declarations() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export  default {}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "const script = {}"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "// export default\nexport default class Foo {}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "// export default\nclass Foo {}\nconst script = Foo"
        );
    }

    #[test]
    fn vue27_rewrite_default_handles_named_default_exports() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "const a = 1 \n export { a as b, a as default, a as c}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "const a = 1 \n export { a as b,  a as c}\nconst script = a"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export { default, foo } from './index.js'",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "import { default as __VUE_DEFAULT__ } from './index.js'\nexport {  foo } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export { foo as default, bar } from './index.js'",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "import { foo } from './index.js'\nexport {  bar } from './index.js'\nconst script = foo"
        );
    }

    #[test]
    fn vue27_rewrite_default_handles_typescript_decorated_classes() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "@Component({})\nexport default class HelloWorld extends Vue {\n  test = \"\";\n}",
                "script",
                Vue27RewriteDefaultOptions {
                    typescript: true,
                    decorators: true,
                },
            ),
            "@Component({})\nclass HelloWorld extends Vue {\n  test = \"\";\n}\nconst script = HelloWorld"
        );
    }

    #[test]
    fn vue3_rewrite_default_handles_official_export_shapes() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "const a = 1",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const a = 1\nconst script = {}"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export default {}",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const script = {}"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export default function Foo() {}",
                    "__default__",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const __default__ = function Foo() {}"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "@Component\nexport default class Foo {}",
                    "script",
                    Vue3RewriteDefaultOptions { typescript: true },
                )
                .unwrap(),
            "@Component class Foo {}\nconst script = Foo"
        );
    }

    #[test]
    fn vue3_rewrite_default_handles_named_default_exports() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "const a = 1 \n export { a as b, a as default, a as c}",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const a = 1 \n export { a as b,  a as c}\nconst script = a"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export { default, foo } from './index.js'",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "import { default as __VUE_DEFAULT__ } from './index.js'\nexport {  foo } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export { foo as default, bar } from './index.js'",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "import { foo as __VUE_DEFAULT__ } from './index.js'\nexport {  bar } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
    }

    #[test]
    fn vue3_rewrite_default_preserves_typescript_plugin_boundary() {
        let compiler = SfcCompiler::new();
        let without_ts = compiler
            .rewrite_vue3_default(
                "export default interface Foo {}",
                "__default__",
                Vue3RewriteDefaultOptions::default(),
            )
            .unwrap_err();
        assert!(without_ts.contains("Unexpected reserved word 'interface'. (1:15)"));

        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export default interface Foo {}",
                    "__default__",
                    Vue3RewriteDefaultOptions { typescript: true },
                )
                .unwrap(),
            "const __default__ = interface Foo {}"
        );
    }
