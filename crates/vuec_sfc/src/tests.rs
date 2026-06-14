#[cfg(test)]
mod tests {
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
        assert!(script.imports.get("defineProps").is_none());
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
        assert!(script.props_aliases.get("foo").is_none());
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
        assert!(script.bindings.get("defineModel").is_none());
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

    #[test]
    fn vue3_compile_script_resolves_relative_imported_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export interface Props { foo: string }",
        )
        .expect("write foo type");
        std::fs::create_dir_all(dir.path().join("bar")).expect("create bar dir");
        std::fs::write(
            dir.path().join("bar").join("index.tsx"),
            "export type ExtraProps = { count?: number }",
        )
        .expect("write bar type");
        std::fs::write(
            dir.path().join("events.d.ts"),
            "type E = { (e: 'save'): void }; export { E as Emits }",
        )
        .expect("write emits type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("unused.ts"),
            "export type Unused = { nope: string }",
        )
        .expect("write unused type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { Props } from './foo'
import { ExtraProps } from './bar'
import type { Emits } from './events'
import type { ModelValue } from './model'
import type { Unused } from './unused'
const props = defineProps<Props & ExtraProps>()
const emit = defineEmits<Emits>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            normalize_path_string(&dir.path().join("foo.ts")),
            normalize_path_string(&dir.path().join("bar").join("index.tsx")),
            normalize_path_string(&dir.path().join("events.d.ts")),
            normalize_path_string(&dir.path().join("model.ts")),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("unused") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_bare_package_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let types_pkg = node_modules.join("vuec-types-pkg");
        let types_dist = types_pkg.join("dist");
        std::fs::create_dir_all(&types_dist).expect("create types package");
        std::fs::write(
            types_pkg.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write types package manifest");
        std::fs::write(
            types_dist.join("index.d.ts"),
            "export interface Props { root: string }\nexport { ExtraProps } from './extra'\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write types package root");
        std::fs::write(
            types_dist.join("extra.d.ts"),
            "export type ExtraProps = { extra?: number }",
        )
        .expect("write types package extra");
        std::fs::write(
            types_dist.join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write types package model");

        let facade_pkg = node_modules.join("vuec-facade-pkg");
        std::fs::create_dir_all(&facade_pkg).expect("create facade package");
        std::fs::write(facade_pkg.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write facade manifest");
        std::fs::write(
            facade_pkg.join("index.d.ts"),
            "export { Props as FacadeProps } from 'vuec-types-pkg'",
        )
        .expect("write facade types");

        let exports_pkg = node_modules.join("vuec-exports-pkg");
        std::fs::create_dir_all(exports_pkg.join("types").join("feature"))
            .expect("create exports package");
        std::fs::write(
            exports_pkg.join("package.json"),
            r#"{"exports":{".":{"types":"./types/index.d.ts","default":"./dist/index.js"},"./feature/*":{"types":"./types/feature/*.d.ts","default":"./dist/feature/*.js"}}}"#,
        )
        .expect("write exports manifest");
        std::fs::write(
            exports_pkg.join("types").join("index.d.ts"),
            "export namespace Nested { export type Props = { flag: boolean } }",
        )
        .expect("write exports root types");
        std::fs::write(
            exports_pkg.join("types").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature: boolean }",
        )
        .expect("write exports feature types");

        let ambient_pkg = node_modules.join("@types").join("vuec-ambient");
        std::fs::create_dir_all(&ambient_pkg).expect("create @types package");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambient: string }",
        )
        .expect("write @types package");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { ExtraProps, Events } from 'vuec-types-pkg'
import type { FacadeProps } from 'vuec-facade-pkg'
import type { FeatureProps } from 'vuec-exports-pkg/feature/item'
import type { AmbientProps } from 'vuec-ambient'
import * as Exported from 'vuec-exports-pkg'
const props = defineProps<FacadeProps & ExtraProps & FeatureProps & AmbientProps & Exported.Nested.Props>()
const emit = defineEmits<Events>()
const model = defineModel<import('vuec-types-pkg').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(
            script
                .content
                .contains("root: { type: String, required: true }"),
            "{}\ndeps: {:?}",
            script.content,
            script.deps
        );
        assert!(script
            .content
            .contains("extra: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("feature: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("ambient: { type: String, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            types_dist.join("index.d.ts"),
            types_dist.join("extra.d.ts"),
            types_dist.join("model.d.ts"),
            facade_pkg.join("index.d.ts"),
            exports_pkg.join("types").join("index.d.ts"),
            exports_pkg.join("types").join("feature").join("item.d.ts"),
            ambient_pkg.join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_package_types_version_selector_supports_node_semver_ranges() {
        for selector in [
            "*",
            "<=5.0",
            "~5.0",
            "^4.8 || >=5.0",
            "5.0 - 5.9",
            ">=4.8 <5.3",
            "5.x",
            "5.*",
        ] {
            assert!(
                vue3_package_types_version_selector_matches(selector),
                "{selector}"
            );
        }

        for selector in ["", ">=5.1", "<5.0", "4.x", "4.*", "5.1 - 5.9"] {
            assert!(
                !vue3_package_types_version_selector_matches(selector),
                "{selector}"
            );
        }
    }

    #[test]
    fn vue3_compile_script_resolves_package_types_versions_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let versioned_pkg = node_modules.join("vuec-typesversions-pkg");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("future").join("feature"))
            .expect("create future types");
        std::fs::create_dir_all(versioned_pkg.join("ts5").join("feature"))
            .expect("create ts5 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["future/index.d.ts"],
                        "feature/*": ["future/feature/*.d.ts"]
                    },
                    "^4.8 || 5.x": {
                        "dist/index.d.ts": ["ts5/index.d.ts"],
                        "feature/*": ["ts5/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface RootProps { fallbackRoot: string }",
        )
        .expect("write fallback root types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface RootProps { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("future").join("index.d.ts"),
            "export interface RootProps { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write future root types");
        std::fs::write(
            versioned_pkg
                .join("future")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { futureFeature: string }",
        )
        .expect("write future feature types");
        std::fs::write(
            versioned_pkg.join("future").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write future model types");
        std::fs::write(
            versioned_pkg.join("ts5").join("index.d.ts"),
            "export interface RootProps { root: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts5 root types");
        std::fs::write(
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature?: number }",
        )
        .expect("write ts5 feature types");
        std::fs::write(
            versioned_pkg.join("ts5").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts5 model types");

        let ambient_pkg = node_modules
            .join("@types")
            .join("vuec-typesversions-ambient");
        std::fs::create_dir_all(ambient_pkg.join("ts5")).expect("create @types versioned");
        std::fs::write(
            ambient_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "~5.0": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write @types package manifest");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambientFallback: number }",
        )
        .expect("write fallback @types");
        std::fs::write(
            ambient_pkg.join("ts5").join("index.d.ts"),
            "export type AmbientProps = { ambient: boolean }",
        )
        .expect("write ts5 @types");

        let type_root_pkg = dir.path().join("typings").join("versioned-global");
        std::fs::create_dir_all(type_root_pkg.join("ts5")).expect("create type root package");
        std::fs::write(
            type_root_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "5.0 - 5.9": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write type root package manifest");
        std::fs::write(
            type_root_pkg.join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRootFallback: number }",
        )
        .expect("write fallback type root global");
        std::fs::write(
            type_root_pkg.join("ts5").join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRoot: string }",
        )
        .expect("write ts5 type root global");

        let ordered_pkg = node_modules.join("vuec-typesversions-ordered");
        std::fs::create_dir_all(ordered_pkg.join("first")).expect("create first ordered types");
        std::fs::create_dir_all(ordered_pkg.join("second")).expect("create second ordered types");
        std::fs::create_dir_all(ordered_pkg.join("fallback"))
            .expect("create fallback ordered types");
        std::fs::write(
            ordered_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    ">=4.8": {
                        "index.d.ts": ["first/index.d.ts"]
                    },
                    ">=5.0": {
                        "index.d.ts": ["second/index.d.ts"]
                    },
                    "*": {
                        "index.d.ts": ["fallback/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write ordered package manifest");
        std::fs::write(
            ordered_pkg.join("index.d.ts"),
            "export type OrderedProps = { orderedFallbackRoot: boolean }",
        )
        .expect("write ordered root fallback");
        std::fs::write(
            ordered_pkg.join("first").join("index.d.ts"),
            "export type OrderedProps = { orderedFirst: string }",
        )
        .expect("write first ordered types");
        std::fs::write(
            ordered_pkg.join("second").join("index.d.ts"),
            "export type OrderedProps = { orderedSecond: number }",
        )
        .expect("write second ordered types");
        std::fs::write(
            ordered_pkg.join("fallback").join("index.d.ts"),
            "export type OrderedProps = { orderedFallback: boolean }",
        )
        .expect("write fallback ordered types");

        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "types": ["versioned-global"],
                    "typeRoots": ["./typings"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'vuec-typesversions-pkg'
import type { FeatureProps } from 'vuec-typesversions-pkg/feature/item'
import type { AmbientProps } from 'vuec-typesversions-ambient'
import type { OrderedProps } from 'vuec-typesversions-ordered'
defineProps<RootProps & FeatureProps & AmbientProps & TypeRootGlobalProps & OrderedProps>()
defineModel<import('vuec-typesversions-pkg').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("feature: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("ambient: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("typeRoot: { type: String, required: true }"));
        assert!(script
            .content
            .contains("orderedFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!script.content.contains("fallbackRoot"));
        assert!(!script.content.contains("futureRoot"));
        assert!(!script.content.contains("futureFeature"));
        assert!(!script.content.contains("legacyRoot"));
        assert!(!script.content.contains("legacyFeature"));
        assert!(!script.content.contains("ambientFallback"));
        assert!(!script.content.contains("typeRootFallback"));
        assert!(!script.content.contains("orderedSecond"));
        assert!(!script.content.contains("orderedFallback"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts5").join("index.d.ts"),
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts5").join("model.d.ts"),
            ambient_pkg.join("ts5").join("index.d.ts"),
            type_root_pkg.join("ts5").join("index.d.ts"),
            ordered_pkg.join("first").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_package_types_versions_from_project_typescript() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let typescript_pkg = node_modules.join("typescript");
        std::fs::create_dir_all(&typescript_pkg).expect("create typescript package");
        std::fs::write(
            typescript_pkg.join("package.json"),
            r#"{"version":"5.2.0"}"#,
        )
        .expect("write typescript manifest");

        let versioned_pkg = node_modules.join("vuec-typesversions-project-ts");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("ts52").join("feature"))
            .expect("create ts52 types");
        std::fs::create_dir_all(versioned_pkg.join("ts50").join("feature"))
            .expect("create ts50 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["ts52/index.d.ts"],
                        "feature/*": ["ts52/feature/*.d.ts"]
                    },
                    ">=5.0": {
                        "dist/index.d.ts": ["ts50/index.d.ts"],
                        "feature/*": ["ts50/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface Props { fallbackRoot: string }",
        )
        .expect("write dist fallback types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface Props { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("index.d.ts"),
            "export interface Props { baselineRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts50 root types");
        std::fs::write(
            versioned_pkg.join("ts50").join("feature").join("item.d.ts"),
            "export type FeatureProps = { baselineFeature: boolean }",
        )
        .expect("write ts50 feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts50 model types");
        std::fs::write(
            versioned_pkg.join("ts52").join("index.d.ts"),
            "export interface Props { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts52 root types");
        std::fs::write(
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            "export type FeatureProps = { futureFeature?: number }",
        )
        .expect("write ts52 feature types");
        std::fs::write(
            versioned_pkg.join("ts52").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write ts52 model types");

        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props } from 'vuec-typesversions-project-ts'
import type { FeatureProps } from 'vuec-typesversions-project-ts/feature/item'
defineProps<Props & FeatureProps>()
defineModel<import('vuec-typesversions-project-ts').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("futureRoot: { type: String, required: true }"));
        assert!(script
            .content
            .contains("futureFeature: { type: Number, required: false }"));
        assert!(script.content.contains("\"modelValue\": { type: Number },"));
        assert!(!script.content.contains("baselineRoot"));
        assert!(!script.content.contains("baselineFeature"));
        assert!(!script.content.contains("legacyRoot"));
        assert!(!script.content.contains("legacyFeature"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts52").join("index.d.ts"),
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts52").join("model.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_tsconfig_path_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let node_modules = dir.path().join("node_modules");
        let types_pkg = node_modules.join("vuec-tsconfig-pkg");
        std::fs::create_dir_all(&types_pkg).expect("create package");
        std::fs::write(types_pkg.join("package.json"), r#"{"types":"index.d.ts"}"#)
            .expect("write package manifest");
        std::fs::write(
            types_pkg.join("index.d.ts"),
            "export type PackageProps = { packaged: boolean }",
        )
        .expect("write package types");

        std::fs::create_dir_all(dir.path().join("web")).expect("create web dir");
        std::fs::create_dir_all(dir.path().join("empty")).expect("create empty dir");
        std::fs::create_dir_all(dir.path().join("tsconfigs")).expect("create tsconfigs dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("src").join("views")).expect("create views dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": [],
                "compilerOptions": {
                    "paths": {
                        "bar": ["./pp.ts"]
                    }
                },
                "references": [
                    { "path": "./tsconfig.app.json" },
                    { "path": "./web" },
                    { "path": "./empty" },
                    { "path": "./noexists-should-ignore" }
                ]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("tsconfig.app.json"),
            r#"{
                "include": ["**/*.ts", "**/*.vue"],
                "extends": ["./tsconfigs/base.json"]
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.path().join("tsconfigs").join("base.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "@/*": ["${configDir}/src/*"]
                    }
                },
                "include": ["${configDir}/src/**/*.ts", "${configDir}/src/**/*.vue"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("web").join("tsconfig.json"),
            r#"{
                "include": ["../**/*.ts", "../**/*.vue"],
                "compilerOptions": {
                    "composite": true,
                    "paths": {
                        "user": ["../user.ts"]
                    }
                },
                "references": [
                    { "path": "../tsconfig.json" }
                ]
            }"#,
        )
        .expect("write web tsconfig");
        std::fs::write(
            dir.path().join("empty").join("tsconfig.json"),
            r#"{"compilerOptions":{"composite":true}}"#,
        )
        .expect("write empty tsconfig");
        std::fs::write(
            dir.path().join("pp.ts"),
            "export type PathProps = { bar: string }",
        )
        .expect("write root path type");
        std::fs::write(
            dir.path().join("user.ts"),
            "export type UserProps = { user: string }",
        )
        .expect("write referenced type");
        std::fs::write(
            dir.path().join("src").join("types.ts"),
            "export type BaseProps = { foo?: string; count: number }",
        )
        .expect("write configDir type");
        std::fs::write(
            dir.path().join("src").join("views").join("Aliased.vue"),
            "<script lang=\"ts\">export type VueProps = { fromVue: string }</script>",
        )
        .expect("write aliased vue");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { PackageProps } from 'vuec-tsconfig-pkg'
import type { PathProps } from 'bar'
import type { UserProps } from 'user'
import type { BaseProps } from '@/types.ts'
import type { VueProps } from '@/views/Aliased.vue'
const props = defineProps<PackageProps & PathProps & UserProps & BaseProps & VueProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for expected_prop in [
            "packaged: { type: Boolean, required: true }",
            "bar: { type: String, required: true }",
            "user: { type: String, required: true }",
            "foo: { type: String, required: false }",
            "count: { type: Number, required: true }",
            "fromVue: { type: String, required: true }",
        ] {
            assert!(script.content.contains(expected_prop), "{}", script.content);
        }

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            types_pkg.join("index.d.ts"),
            dir.path().join("pp.ts"),
            dir.path().join("user.ts"),
            dir.path().join("src").join("types.ts"),
            dir.path().join("src").join("views").join("Aliased.vue"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_tsconfig_jsonc_paths_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("src").join("base")).expect("create base dir");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                // Root path mapping.
                "compilerOptions": {
                    "paths": {
                        "root-alias": ["./root.ts",],
                    },
                },
                "references": [
                    { "path": "./tsconfig.app.json", },
                ],
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("tsconfig.app.json"),
            r#"{
                "extends": [
                    "./config/base.json", // inherited alias
                ],
                "compilerOptions": {
                    "paths": {
                        "app-alias": ["./app.ts",],
                    },
                },
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                /* ${configDir} should still resolve from the referencing config. */
                "compilerOptions": {
                    "paths": {
                        "@base/*": ["${configDir}/src/base/*",],
                    },
                },
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.path().join("app.ts"),
            "export type AppProps = { app?: number }",
        )
        .expect("write app type");
        std::fs::write(
            dir.path().join("src").join("base").join("types.ts"),
            "export type BaseProps = { base: boolean }",
        )
        .expect("write base type");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'root-alias'
import type { AppProps } from 'app-alias'
import type { BaseProps } from '@base/types'
defineProps<RootProps & AppProps & BaseProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("app: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("base: { type: Boolean, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("root.ts"),
            dir.path().join("app.ts"),
            dir.path().join("src").join("base").join("types.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_package_tsconfig_extends_paths_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        let scoped_config_pkg = dir
            .path()
            .join("node_modules")
            .join("@vuec")
            .join("tsconfig");
        std::fs::create_dir_all(&scoped_config_pkg).expect("create scoped config package");
        std::fs::write(
            scoped_config_pkg.join("package.json"),
            r#"{"tsconfig":"base.json"}"#,
        )
        .expect("write scoped config package manifest");
        std::fs::write(
            scoped_config_pkg.join("base.json"),
            r#"{
                // Package config entries may be JSONC.
                "compilerOptions": {
                    "paths": {
                        "pkg-root": ["${configDir}/root.ts",],
                    },
                },
            }"#,
        )
        .expect("write scoped package config");

        let preset_pkg = dir.path().join("node_modules").join("vuec-tsconfig-preset");
        std::fs::create_dir_all(&preset_pkg).expect("create preset package");
        std::fs::write(
            preset_pkg.join("shared.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "pkg-shared": ["${configDir}/shared.ts"]
                    }
                }
            }"#,
        )
        .expect("write preset subpath config");

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": ["@vuec/tsconfig", "vuec-tsconfig-preset/shared"],
                "compilerOptions": {
                    "paths": {
                        "local-alias": ["./local.ts"]
                    }
                }
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.path().join("shared.ts"),
            "export type SharedProps = { shared?: number }",
        )
        .expect("write shared type");
        std::fs::write(
            dir.path().join("local.ts"),
            "export type LocalProps = { local: boolean }",
        )
        .expect("write local type");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { RootProps } from 'pkg-root'
import type { SharedProps } from 'pkg-shared'
import type { LocalProps } from 'local-alias'
defineProps<RootProps & SharedProps & LocalProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("shared: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("local: { type: Boolean, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("root.ts"),
            dir.path().join("shared.ts"),
            dir.path().join("local.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_tsconfig_jsonc_preserves_string_literal_contents() {
        let value = vue3_parse_tsconfig_jsonc(
            r#"{
                "compilerOptions": {
                    "baseUrl": "./src,not-trailing",
                    "paths": {
                        "url/*": [
                            "./literal//slash/*",
                            "./literal/*block*/segment/*",
                        ],
                    },
                },
            }"#,
        )
        .expect("parse jsonc tsconfig");
        let compiler_options = value
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
            .expect("compiler options");
        assert_eq!(
            compiler_options
                .get("baseUrl")
                .and_then(serde_json::Value::as_str),
            Some("./src,not-trailing")
        );
        let targets = compiler_options
            .get("paths")
            .and_then(|paths| paths.get("url/*"))
            .and_then(serde_json::Value::as_array)
            .expect("paths target");
        let targets = targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            targets,
            vec!["./literal//slash/*", "./literal/*block*/segment/*"]
        );
    }

    #[test]
    fn vue3_compile_script_resolves_relative_re_exported_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export type LeafProps = { leaf?: number }",
        )
        .expect("write leaf type");
        std::fs::write(
            dir.path().join("events.ts"),
            "export type LeafEmits = { (e: 'save'): void }",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("bar.ts"),
            "export { LeafProps as BarProps } from './leaf'\nexport * from './events'",
        )
        .expect("write bar type");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export { BarProps as Props } from './bar'\nexport { LeafEmits as Emits } from './bar'\nexport * from './model'",
        )
        .expect("write foo type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props, Emits, ModelValue } from './foo'
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("leaf: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["foo.ts", "bar.ts", "leaf.ts", "events.ts", "model.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_default_type_imports_and_re_exports() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("direct_base.ts"),
            "export interface DirectBase { inherited?: string }",
        )
        .expect("write direct base type");
        std::fs::write(
            dir.path().join("direct.ts"),
            "import type { DirectBase } from './direct_base'\nexport default interface DirectProps extends DirectBase { direct?: boolean }",
        )
        .expect("write direct type");
        std::fs::write(
            dir.path().join("alias.ts"),
            "type AliasProps = { alias: string }; export default AliasProps",
        )
        .expect("write alias type");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export default interface LeafProps { leaf: string }",
        )
        .expect("write leaf type");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { default } from './leaf'",
        )
        .expect("write default facade");
        std::fs::write(
            dir.path().join("named.ts"),
            "export interface NamedProps { named: number }",
        )
        .expect("write named type");
        std::fs::write(
            dir.path().join("default_named.ts"),
            "export { NamedProps as default } from './named'",
        )
        .expect("write named default facade");
        std::fs::write(
            dir.path().join("renamed.ts"),
            "export { default as RenamedProps } from './alias'",
        )
        .expect("write renamed default facade");
        std::fs::write(
            dir.path().join("events.ts"),
            "type Events = { (e: 'save'): void }; export default Events",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("model.ts"),
            "type ModelValue = boolean | string; export default ModelValue",
        )
        .expect("write model type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import DirectProps from './direct'
import AliasProps from './alias'
import FacadeProps from './facade'
import NamedDefaultProps from './default_named'
import { RenamedProps } from './renamed'
import Events from './events'
import ModelValue from './model'
const props = defineProps<DirectProps & AliasProps & FacadeProps & NamedDefaultProps & RenamedProps>()
const emit = defineEmits<Events>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("direct: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("inherited: { type: String, required: false }"));
        assert!(script
            .content
            .contains("alias: { type: String, required: true }"));
        assert!(script
            .content
            .contains("leaf: { type: String, required: true }"));
        assert!(script
            .content
            .contains("named: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "direct_base.ts",
            "direct.ts",
            "alias.ts",
            "leaf.ts",
            "facade.ts",
            "named.ts",
            "default_named.ts",
            "renamed.ts",
            "events.ts",
            "model.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_class_declaration_types_and_default_class_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("classes.ts"),
            "export class NamedClass {}\nexport type Props = { named: NamedClass }\nexport default class DefaultClass {}",
        )
        .expect("write class types");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export default class LeafClass {}",
        )
        .expect("write default class leaf");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { default } from './leaf'",
        )
        .expect("write default class facade");
        std::fs::write(
            dir.path().join("named_facade.ts"),
            "export { NamedClass as RenamedClass } from './classes'",
        )
        .expect("write named class facade");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            "declare type GlobalProps = { global: GlobalClass }\ndeclare class GlobalClass {}",
        )
        .expect("write global class types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import DefaultClass from './facade'
import { NamedClass, Props } from './classes'
import { RenamedClass } from './named_facade'
type LocalProps = { local: LocalClass, defaulted: DefaultClass, named: NamedClass, renamed: RenamedClass, props: Props }
class LocalClass {}
const props = defineProps<LocalProps & GlobalProps>()
const model = defineModel<DefaultClass>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        for prop in ["local", "defaulted", "named", "renamed", "props", "global"] {
            assert!(
                script
                    .content
                    .contains(&format!("{prop}: {{ type: Object, required: true }}")),
                "{}",
                script.content
            );
        }
        assert!(script.content.contains("\"modelValue\": { type: Object },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("classes.ts"),
            dir.path().join("leaf.ts"),
            dir.path().join("facade.ts"),
            dir.path().join("named_facade.ts"),
            global,
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_enum_macro_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("enums.ts"),
            "export enum Kind { A = 'a', B = 'b' }\nexport enum Code { A = 1, B = 2 }\nexport enum Mixed { A = 'a', B = 1 }\nexport enum Auto { A, B }\nexport type Props = { kind: Kind, code?: Code, mixed: Mixed, auto: Auto }\nexport type ModelValue = Kind | Code",
        )
        .expect("write enum types");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { Props as FacadeProps, ModelValue as FacadeModel } from './enums'",
        )
        .expect("write enum facade");
        std::fs::write(
            dir.path().join("namespace.ts"),
            "export namespace Nested { export enum Flag { Yes = 'yes', No = 'no' } export type Props = { flag: Flag } }",
        )
        .expect("write namespace enum types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { FacadeProps, FacadeModel } from './facade'
import * as Ns from './namespace'
const props = defineProps<FacadeProps & Ns.Nested.Props>()
const model = defineModel<FacadeModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("kind: { type: String, required: true }"));
        assert!(script
            .content
            .contains("code: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("mixed: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("auto: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["enums.ts", "facade.ts", "namespace.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_dynamic_import_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.ts"),
            "export type Props = { foo: string, bar: import('./bar').N }",
        )
        .expect("write props type");
        std::fs::write(dir.path().join("bar.ts"), "export type N = number")
            .expect("write prop leaf type");
        std::fs::write(
            dir.path().join("events.ts"),
            "export type Events = import('./event_leaf').Events",
        )
        .expect("write events type");
        std::fs::write(
            dir.path().join("event_leaf.ts"),
            "export type Events = { (e: 'save'): void }",
        )
        .expect("write event leaf type");
        std::fs::write(
            dir.path().join("model.ts"),
            "export type ModelValue = import('./model_leaf').ModelValue",
        )
        .expect("write model type");
        std::fs::write(
            dir.path().join("model_leaf.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model leaf type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
const props = defineProps<import('./foo').Props>()
const emit = defineEmits<import('./events').Events>()
const model = defineModel<import('./model').ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "foo.ts",
            "bar.ts",
            "events.ts",
            "event_leaf.ts",
            "model.ts",
            "model_leaf.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_namespace_imported_types_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("types.ts"),
            "export type Props = { foo: string }\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = boolean | string\nexport type Unused = { nope: string }",
        )
        .expect("write namespace types");
        std::fs::write(
            dir.path().join("leaf.ts"),
            "export namespace Nested { export type ExtraProps = { count?: number } }",
        )
        .expect("write nested namespace types");
        std::fs::write(
            dir.path().join("dynamic.ts"),
            "export namespace Types { export type Props = { bar: number } }",
        )
        .expect("write dynamic namespace types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import * as Types from './types'
import * as Leaf from './leaf'
const props = defineProps<Types.Props & Leaf.Nested.ExtraProps & import('./dynamic').Types.Props>()
const emit = defineEmits<Types.Events>()
const model = defineModel<Types.ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("bar: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["types.ts", "leaf.ts", "dynamic.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("unused") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_relative_vue_type_imports_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.vue"),
            "<template><div /></template><script lang=\"ts\">export type Props = { foo: number }</script>",
        )
        .expect("write foo vue");
        std::fs::write(
            dir.path().join("bar.vue"),
            "<script setup lang=\"tsx\">export type ExtraProps = { bar: string }</script>",
        )
        .expect("write bar vue");
        std::fs::write(
            dir.path().join("events.vue"),
            "<script setup lang=\"ts\">export type Events = { (e: 'save'): void }</script>",
        )
        .expect("write events vue");
        std::fs::write(
            dir.path().join("model.vue"),
            "<script lang=\"ts\">export type ModelValue = boolean | string</script>",
        )
        .expect("write model vue");
        std::fs::write(
            dir.path().join("leaf.vue"),
            "<script setup lang=\"ts\">export type LeafProps = { leaf?: boolean }</script>",
        )
        .expect("write leaf vue");
        std::fs::write(
            dir.path().join("facade.ts"),
            "export { LeafProps } from './leaf.vue'",
        )
        .expect("write facade");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { Props } from './foo.vue'
import { ExtraProps } from './bar.vue'
import { LeafProps } from './facade'
import { Events } from './events.vue'
import { ModelValue } from './model.vue'
const props = defineProps<Props & ExtraProps & LeafProps>()
const emit = defineEmits<Events>()
const model = defineModel<ModelValue>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: String, required: true }"));
        assert!(script
            .content
            .contains("leaf: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "foo.vue",
            "bar.vue",
            "facade.ts",
            "leaf.vue",
            "events.vue",
            "model.vue",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_arbitrary_extension_type_sidecars() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("foo.d.vue.ts"),
            "export type FooProps = { foo: number }",
        )
        .expect("write vue sidecar");
        std::fs::write(dir.path().join("foo.vue"), "<template><div /></template>")
            .expect("write foo vue");
        std::fs::write(
            dir.path().join("bar.d.css.ts"),
            "export type BarProps = { bar: string }",
        )
        .expect("write css sidecar");
        std::fs::write(dir.path().join("bar.css"), ".bar { color: red; }").expect("write css");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { FooProps } from './foo.vue'
import { BarProps } from './bar.css'
defineProps<FooProps & BarProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: String, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["foo.d.vue.ts", "bar.d.css.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_extract_prop_types_from_declared_const_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare const props: {
  foo: StringConstructor
  bar: { type: import('foo').EpPropFinalized<BooleanConstructor>, required: true }
}
type Props = ExtractPropTypes<typeof props>
const resolved = defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("bar: { type: Boolean, required: true }"));
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
    fn vue3_compile_script_resolves_extract_prop_types_from_runtime_props_object() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("user.ts"),
            "export interface User { id: string }",
        )
        .expect("write user type");
        std::fs::write(
            dir.path().join("props.ts"),
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
            dir.path().join("default-props.ts"),
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
            dir.path().join("direct-default-props.ts"),
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

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { props as namedProps } from './props'
import defaultProps from './default-props'
import directDefaultProps from './direct-default-props'
type Props =
  ExtractPropTypes<typeof namedProps> &
  Partial<ExtractPropTypes<typeof defaultProps>> &
  ExtractPropTypes<typeof directDefaultProps>
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("name: { type: String, required: false }"));
        assert!(script
            .content
            .contains("active: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("score: { type: [Number, String], required: false }"));
        assert!(script
            .content
            .contains("user: { type: Object, required: false }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("created: { type: Date, required: false }"));
        assert!(script
            .content
            .contains("direct: { type: String, required: true }"));
        assert!(script
            .content
            .contains("owner: { type: Object, required: false }"));
        assert!(script
            .content
            .contains("mode: { type: [Boolean, Number], required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            "user.ts",
            "props.ts",
            "default-props.ts",
            "direct-default-props.ts",
        ]
        .into_iter()
        .map(|name| normalize_path_string(&dir.path().join(name)))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_partial_import_extract_prop_types_return_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare const props: () => {
  foo: StringConstructor
  active: { type: BooleanConstructor, required: true }
}
type Props = Partial<import('vue').ExtractPropTypes<ReturnType<typeof props>>>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("active: { type: Boolean, required: false }"));
        assert_eq!(
            script.bindings.get("active").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_pick_omit_utility_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Base = { foo: number, bar: string, baz?: boolean }
type Keys = 'foo' | 'bar'
type Props = Pick<Base, Keys> & Partial<Omit<Base, Keys>>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: String, required: true }"));
        assert!(script
            .content
            .contains("baz: { type: Boolean, required: false }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_merges_duplicate_union_intersection_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Left = { shared: string; left?: boolean }
type Right = { shared?: number; right: Function }
type Props = Left & Right & ({ variant: string } | { variant?: boolean })
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert_eq!(script.content.matches("shared: {").count(), 1);
        assert_eq!(script.content.matches("variant: {").count(), 1);
        assert!(script
            .content
            .contains("shared: { type: [String, Number], required: false }"));
        assert!(script
            .content
            .contains("left: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("right: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("variant: { type: [String, Boolean], required: false }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_generic_props_type_aliases() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Foo = { foo: string }
type Bar = { bar?: number }
type Props<T, U> = Readonly<T & U & { baz: boolean }>
type Box<T> = { value: T }
type Optional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>
interface Test { one: string; two: string }
defineProps<Props<Foo, Bar> & Box<string> & Optional<Test, 'one'> & Pick<Test, keyof Test>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("baz: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("value: { type: String, required: true }"));
        assert!(script
            .content
            .contains("one: { type: String, required: false }"));
        assert!(script
            .content
            .contains("two: { type: String, required: true }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_mapped_template_literal_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Flag = 'foo' | 'bar'
type Breakpoints = 'sm' | 'md' | 'lg'
type BreakpointFactory<T extends string, V> = {
  [K in Breakpoints as `${T}${Capitalize<K>}`]?: V
}
type Props = {
  [K in `_${Flag}_${Breakpoints}_`]: string
} & {
  [K in Flag as `${K}_flag`]: boolean
} & {
  [K in Uppercase<Extract<Flag, 'foo'>> as `upper${Capitalize<Lowercase<K>>}`]?: string
} & {
  [K in Lowercase<'LOUD'> as `${K}_lower`]: number
} & {
  [K in Uncapitalize<'Title'> as `${K}_name`]: boolean
} & {
  [K in Exclude<Flag, 'bar'> as `${K}_only`]: string
} & BreakpointFactory<'cols', number>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("_foo_sm_: { type: String, required: true }"));
        assert!(script
            .content
            .contains("_foo_md_: { type: String, required: true }"));
        assert!(script
            .content
            .contains("_bar_lg_: { type: String, required: true }"));
        assert!(script
            .content
            .contains("foo_flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("bar_flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("upperFoo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("loud_lower: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("title_name: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("foo_only: { type: String, required: true }"));
        assert!(script
            .content
            .contains("colsSm: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("colsMd: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("colsLg: { type: Number, required: false }"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_record_props_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Flag = 'foo' | 'bar'
type Breakpoints = 'sm' | 'md'
type Props =
  Record<`${Flag}_${Breakpoints}`, number> &
  Partial<Record<Uppercase<Extract<Flag, 'foo'>>, string>> &
  Record<Exclude<Flag, 'bar'>, boolean>
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo_sm: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("foo_md: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar_sm: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("bar_md: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("FOO: { type: String, required: false }"));
        assert!(script
            .content
            .contains("foo: { type: Boolean, required: true }"));
        assert_eq!(
            script.bindings.get("foo_sm").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("FOO").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_indexed_access_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Base = {
  name: string
  count?: number
  active: boolean
  method(): void
  run: () => void
}
type A = (string | number)[]
type AA = Array<string>
type T = [1, 'foo']
type TT = [foo: 1, bar: 'foo']
type ValueOf<T, K extends keyof T> = T[K]
type Props = {
  directMethod(): void
  label: Base['name']
  scalar: Base['name' | 'count']
  method: Base['method']
  callable: Base['run']
  methodOrCallable: Base['method'] | Base['run']
  methodOrLabel: Base['method'] | Base['name']
  generic: ValueOf<Base, 'active'>
  arrayItem: A[number]
  genericArrayItem: AA[number]
  tupleItem: T[number]
  namedTupleItem: TT[number]
}
defineProps<Props>()
defineModel<A[number] | TT[number] | Base['method'] | Base['run']>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("directMethod: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("scalar: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("method: { type: null, required: true }"));
        assert!(script
            .content
            .contains("callable: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("methodOrCallable: { type: Function, required: true, skipCheck: true }"));
        assert!(script
            .content
            .contains("methodOrLabel: { type: null, required: true }"));
        assert!(script
            .content
            .contains("generic: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("arrayItem: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("genericArrayItem: { type: String, required: true }"));
        assert!(script
            .content
            .contains("tupleItem: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("namedTupleItem: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Function], skipCheck: true },"));
        assert_eq!(
            script.bindings.get("label").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("generic").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_parameter_tuple_utility_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Fn = (value: string, count: number, active?: boolean) => void
type Ctor = new (name: string, flags: boolean[]) => object
type Props = {
  first: Parameters<Fn>[0]
  anyParam: Parameters<Fn>[number]
  ctorFirst: ConstructorParameters<Ctor>[0]
  ctorAny: ConstructorParameters<Ctor>[number]
  inlineParam: Parameters<(files: File[], done: () => void) => void>[number]
}
defineProps<Props>()
defineModel<Parameters<Fn>[number] | ConstructorParameters<Ctor>[number]>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("first: { type: String, required: true }"));
        assert!(script
            .content
            .contains("anyParam: { type: [String, Number, Boolean], required: true }"));
        assert!(script
            .content
            .contains("ctorFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("ctorAny: { type: [String, Array], required: true }"));
        assert!(script
            .content
            .contains("inlineParam: { type: [Array, Function], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean, Array] },"));
        assert_eq!(
            script.bindings.get("first").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("ctorAny").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_signature_parameter_tuple_utility_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Callable = {
  (value: string, count: number): void
  (active: boolean): void
}
interface InterfaceCallable {
  (name: string, flags: boolean[]): void
}
type Newable = {
  new (id: number, done: () => void): object
}
interface InterfaceNewable {
  new (label: string, enabled: boolean): object
}
type Props = {
  callAny: Parameters<Callable>[number]
  callFirst: Parameters<InterfaceCallable>[0]
  newAny: ConstructorParameters<Newable>[number]
  newSecond: ConstructorParameters<InterfaceNewable>[1]
}
defineProps<Props>()
defineModel<Parameters<Callable>[number] | ConstructorParameters<InterfaceNewable>[number]>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("callAny: { type: [String, Boolean, Number], required: true }"));
        assert!(script
            .content
            .contains("callFirst: { type: String, required: true }"));
        assert!(script
            .content
            .contains("newAny: { type: [Number, Function], required: true }"));
        assert!(script
            .content
            .contains("newSecond: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(
            script.bindings.get("callFirst").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("newSecond").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_interface_extends_signature_parameter_tuple_utility_runtime_types(
    ) {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Callable extends BaseCallable {
  (active: boolean): void
}
interface BaseCallable {
  (name: string, count: number): void
}
interface Newable extends BaseNewable {
  new (label: string): object
}
interface BaseNewable {
  new (id: number, done: () => void): object
}
type Props = {
  callAny: Parameters<Callable>[number]
  callSecond: Parameters<Callable>[1]
  newAny: ConstructorParameters<Newable>[number]
  newSecond: ConstructorParameters<Newable>[1]
}
defineProps<Props>()
defineModel<Parameters<Callable>[number] | ConstructorParameters<Newable>[number]>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("callAny: { type: [Boolean, String, Number], required: true }"));
        assert!(script
            .content
            .contains("callSecond: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("newAny: { type: [String, Number, Function], required: true }"));
        assert!(script
            .content
            .contains("newSecond: { type: Function, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String, Number, Function] },"));
        assert_eq!(
            script.bindings.get("callAny").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("newSecond").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_return_type_utility_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
declare function makeLabel(): string
declare const makeCount: () => number
type BooleanFactory = () => boolean
type Callable = {
  (value: string): Date
  (value: number): Error
}
interface InterfaceFactory {
  (active: boolean): string[]
}
interface ExtendedFactory extends InterfaceFactory {
  (value: number): boolean
}
type Props = {
  label: ReturnType<typeof makeLabel>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<BooleanFactory>
  mixed: ReturnType<Callable>
  list: ReturnType<InterfaceFactory>
  extended: ReturnType<ExtendedFactory>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeLabel> | ReturnType<BooleanFactory>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("mixed: { type: [Date, Error], required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("extended: { type: [Boolean, Array], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean] },"));
        assert_eq!(
            script.bindings.get("label").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("extended").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_external_default_function_return_type_runtime_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("named.ts"),
            "export default function makeDefault(): string { return '' }\nexport function makeCount(): number { return 1 }",
        )
        .expect("write named default function type");
        std::fs::write(
            dir.path().join("anonymous.ts"),
            "export default function(): boolean { return true }",
        )
        .expect("write anonymous default function type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import makeDefault, { makeCount } from './named'
import makeFlag from './anonymous'
type Props = {
  label: ReturnType<typeof makeDefault>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<typeof makeFlag>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeDefault> | ReturnType<typeof makeCount> | ReturnType<typeof makeFlag>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["named.ts", "anonymous.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_external_function_value_return_type_runtime_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("factories.ts"),
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
            dir.path().join("arrow-default.ts"),
            "export default ((): Date => new Date())",
        )
        .expect("write default arrow function value");
        std::fs::write(
            dir.path().join("function-default.ts"),
            "export default (function(): Error { return new Error() })",
        )
        .expect("write default function expression value");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import makeDate from './arrow-default'
import makeError from './function-default'
import { makeLabel, makeCount, makeFlag } from './factories'
type Props = {
  label: ReturnType<typeof makeLabel>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<typeof makeFlag>
  date: ReturnType<typeof makeDate>
  error: ReturnType<typeof makeError>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeLabel> | ReturnType<typeof makeCount> | ReturnType<typeof makeFlag> | ReturnType<typeof makeDate> | ReturnType<typeof makeError>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("date: { type: Date, required: true }"));
        assert!(script
            .content
            .contains("error: { type: Error, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean, Date, Error] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["factories.ts", "arrow-default.ts", "function-default.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_unannotated_function_return_runtime_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("factories.ts"),
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
            dir.path().join("date.ts"),
            "export default function makeDate() { return new Date() }",
        )
        .expect("write default unannotated function");
        std::fs::write(
            dir.path().join("error.ts"),
            "export default (function() { return new Error('x') })",
        )
        .expect("write default unannotated function expression");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import makeDate from './date'
import makeError from './error'
import { makeLabel, makeCount, makeFlag, makeList, makeBox } from './factories'
type Props = {
  label: ReturnType<typeof makeLabel>
  count: ReturnType<typeof makeCount>
  flag: ReturnType<typeof makeFlag>
  list: ReturnType<typeof makeList>
  box: ReturnType<typeof makeBox>
  made: ReturnType<typeof import('./factories').makeFlag>
  created: ReturnType<typeof makeDate>
  error: ReturnType<typeof makeError>
}
defineProps<Props>()
defineModel<ReturnType<typeof makeLabel> | ReturnType<typeof makeCount> | ReturnType<typeof makeFlag> | ReturnType<typeof makeList> | ReturnType<typeof makeBox> | ReturnType<typeof makeDate> | ReturnType<typeof makeError>>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("box: { type: Object, required: true }"));
        assert!(script
            .content
            .contains("made: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("created: { type: Date, required: true }"));
        assert!(script
            .content
            .contains("error: { type: Error, required: true }"));
        assert!(script.content.contains(
            "\"modelValue\": { type: [String, Number, Boolean, Array, Object, Date, Error] },"
        ));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = ["factories.ts", "date.ts", "error.ts"]
            .into_iter()
            .map(|name| normalize_path_string(&dir.path().join(name)))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_merged_type_declarations() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
interface Foo {
  a: string
}
interface Foo {
  b: number
}
namespace Bar {
  export type A = string
}
namespace Bar {
  export type B = number
}
namespace Baz {
  export type A = string
}
interface Baz {
  b: number
}
enum Kind {
  A = 1
}
enum Kind {
  B = 'hi'
}
type Props = {
  foo: Foo['a']
  bar: Foo['b']
  nsA: Bar.A
  nsB: Bar.B
  mixedNs: Baz.A
  mixedInterface: Baz['b']
  kind: Kind
}
defineProps<Props>()
defineModel<Kind>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("nsA: { type: String, required: true }"));
        assert!(script
            .content
            .contains("nsB: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("mixedNs: { type: String, required: true }"));
        assert!(script
            .content
            .contains("mixedInterface: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("kind: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(
            script.bindings.get("mixedInterface").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_reports_props_type_resolution_errors() {
        let mut compiler = SfcCompiler::new();

        let unresolved = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
defineProps<X>()
</script>"#,
        );
        let script = compiler.compile_script(&unresolved, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("Unresolvable type reference or unsupported built-in utility type")
        }));

        let computed = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
defineProps<{ [Foo]: string }>()
</script>"#,
        );
        let script = compiler.compile_script(&computed, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| {
            error.contains("Unsupported computed key in type referenced by a macro")
        }));

        let indexed = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
defineProps<X[K]>()
</script>"#,
        );
        let script = compiler.compile_script(&indexed, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Unsupported type when resolving index type")));

        let missing_import = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
import { X } from './foo'
defineProps<X>()
</script>"#,
        );
        let script = compiler.compile_script(&missing_import, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Failed to resolve import source \"./foo\".")));

        let member_runtime = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
import type P from 'unknown'
defineProps<{ foo: T, bar: T['bar'], baz: P }>()
</script>"#,
        );
        let script = compiler.compile_script(&member_runtime, SfcScriptCompileOptions::default());
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
            script.bindings.get("baz").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_resolves_top_level_indexed_access_props_type() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type T = { bar: number }
type S = { nested: { foo: T['bar'] } }
defineProps<S['nested']>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("foo: { type: Number, required: true }"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_resolves_runtime_utility_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type MaybeText = string | null
type Props = {
  label: NonNullable<MaybeText>
  extracted: Extract<string | number | boolean, number | boolean>
  excluded: Exclude<string | number, number>
}
defineProps<Props>()
defineModel<NonNullable<string | null> | Extract<number | boolean, boolean> | Exclude<string | number, number>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: true }"));
        assert!(script
            .content
            .contains("extracted: { type: [Number, Boolean], required: true }"));
        assert!(script
            .content
            .contains("excluded: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(
            script.bindings.get("label").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("extracted").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_static_conditional_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Runtime<T> = T extends 'text' ? string : T extends 'count' ? number : boolean
type Props = {
  directTrue: 'on' extends 'on' ? boolean : string
  directFalse: 'off' extends 'on' ? boolean : string
  text: Runtime<'text'>
  count: Runtime<'count'>
  active: Runtime<'active'>
  unresolved: Runtime<'text' | 'count'>
}
defineProps<Props>()
defineModel<Runtime<'text'> | Runtime<'count'> | Runtime<'active'>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("directTrue: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("directFalse: { type: String, required: true }"));
        assert!(script
            .content
            .contains("text: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("active: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("unresolved: { type: null, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Number, Boolean] },"));
        assert_eq!(
            script.bindings.get("text").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_bigint_literal_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Big = 1n
type Props = {
  literal: 1n
  union: 1n | 'text'
  alias: Big
  keyword: bigint
}
defineProps<Props>()
defineModel<1n | 'text'>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("literal: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("union: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("alias: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("keyword: { type: null, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(
            script.bindings.get("literal").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_builtin_wrapper_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Props = {
  list: ReadonlyArray<string>
  params: Parameters<(value: string) => void>
  ctorParams: ConstructorParameters<any>
  map: ReadonlyMap<string, number>
  set: ReadonlySet<string>
  err: Error
  loud: Uppercase<'foo'>
  maybe: MaybeRef<string[]>
  getter: MaybeRefOrGetter<boolean>
  ref: Ref<number>
}
defineProps<Props>()
defineModel<ReadonlyArray<string> | ReadonlyMap<string, number> | ReadonlySet<string> | Error | MaybeRefOrGetter<boolean> | Parameters<() => void> | Uppercase<'foo'>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("params: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("ctorParams: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("map: { type: Map, required: true }"));
        assert!(script
            .content
            .contains("set: { type: Set, required: true }"));
        assert!(script
            .content
            .contains("err: { type: Error, required: true }"));
        assert!(script
            .content
            .contains("loud: { type: String, required: true }"));
        assert!(script
            .content
            .contains("maybe: { type: [Object, Array], required: true }"));
        assert!(script
            .content
            .contains("getter: { type: [Object, Function, Boolean], required: true }"));
        assert!(script
            .content
            .contains("ref: { type: Object, required: true }"));
        assert!(script.content.contains(
            "\"modelValue\": { type: [Array, Map, Set, Error, Object, Function, Boolean, String] },"
        ));
        assert_eq!(
            script.bindings.get("getter").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_mapped_identity_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type RuntimeMirror<T> = { [K in keyof T]: T[K] }
type Props = {
  label: RuntimeMirror<string | number>
  boxed: RuntimeMirror<{ value: boolean }>
  list: RuntimeMirror<ReadonlyArray<string>>
}
defineProps<Props>()
defineModel<RuntimeMirror<string | boolean>>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("boxed: { type: Object, required: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [String, Boolean] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_type_operator_runtime_types() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Base = {
  name: string
  1: boolean
}
type Props = {
  readonlyList: readonly string[]
  objectKeys: keyof Base
  literalKeys: keyof { [index: number]: string; label: string }
  arrayKeys: keyof ReadonlyArray<string>
  anyKeys: keyof any
  pickedKeys: keyof Pick<Base, 'name'>
}
defineProps<Props>()
defineModel<readonly boolean[] | keyof any>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("readonlyList: { type: Array, required: true }"));
        assert!(script
            .content
            .contains("objectKeys: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("literalKeys: { type: [Number, String], required: true }"));
        assert!(script
            .content
            .contains("arrayKeys: { type: [String, Number], required: true }"));
        assert!(script
            .content
            .contains("anyKeys: { type: [String, Number, Symbol], required: true }"));
        assert!(script
            .content
            .contains("pickedKeys: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Array, String, Number, Symbol] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

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
        assert!(ignored_script.bindings.get("skipped").is_none());
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

    #[test]
    fn vue3_compile_script_resolves_forward_type_alias_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<script setup lang="ts">
type Props = Mid & {
  own: string
}
type Mid = Base & {
  mid?: boolean
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
            .contains("mid: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("inherited: { type: Number, required: false }"));
        assert_eq!(
            script.bindings.get("own").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("mid").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("inherited").map(String::as_str),
            Some("props")
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_external_forward_type_alias_props_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let types_file = dir.path().join("types.ts");
        std::fs::write(
            &types_file,
            "export type Props = Base & { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write type alias props");

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
            .contains("local: { type: Number, required: true }"));
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
    fn vue3_compile_script_resolves_external_declared_return_type_extract_prop_types_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let props_file = dir.path().join("upload.ts");
        std::fs::write(
            &props_file,
            concat!(
                "import type { PropType } from 'vue'\n",
                "export interface UploadFile<T> { raw: T }\n",
                "export declare function uploadProps<T>(): {\n",
                "  fileList: { type: PropType<UploadFile<T>[]>, default: UploadFile<T>[] }\n",
                "}\n"
            ),
        )
        .expect("write upload props type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import { uploadProps } from './upload'
type Props = ExtractPropTypes<ReturnType<typeof uploadProps>>
defineProps<Props>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("fileList: { type: Array, required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [props_file]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_local_type_shadows_imported_type_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("props.ts"),
            "export type Props = { imported: string }\nexport enum Kind { Imported = 'x' }",
        )
        .expect("write props type");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
import type { Props, Kind } from './props'
type Props = { local: number }
enum Kind { Local = 1 }
defineProps<Props>()
defineModel<Kind>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("local: { type: Number, required: true }"));
        assert!(!script.content.contains("imported: { type: String"));
        assert!(script.content.contains("\"modelValue\": { type: Number },"));
        assert!(!script
            .content
            .contains("\"modelValue\": { type: [String, Number] },"));
        assert!(script.deps.is_empty(), "{:?}", script.deps);
    }

    #[test]
    fn vue3_compile_script_resolves_global_type_files_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            "declare interface GlobalProps { msg: string; count?: number }\ndeclare type GlobalEmits = { (e: 'save'): void }\ndeclare type GlobalModel = boolean | string",
        )
        .expect("write ambient global types");
        let module_global = dir.path().join("module-global.d.ts");
        std::fs::write(
            &module_global,
            "export {}\ndeclare global { interface AugmentedProps { flag: boolean } }",
        )
        .expect("write module global types");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<GlobalProps & AugmentedProps>()
defineEmits<GlobalEmits>()
defineModel<GlobalModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![
                    global.to_string_lossy().to_string(),
                    module_global.to_string_lossy().to_string(),
                ],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("msg: { type: String, required: true }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("flag: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global, module_global]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_discovers_tsconfig_global_type_files_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("types").join("nested")).expect("create types dir");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::create_dir_all(dir.path().join("project")).expect("create project dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "files": ["./types/root.d.ts"],
                "include": ["./types/**/*.ts", "./src/**/*.vue"],
                "extends": "./config/base.json",
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                "files": ["${configDir}/types/base.d.ts"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.path().join("project").join("tsconfig.json"),
            r#"{
                "files": ["../types/ref.d.ts"]
            }"#,
        )
        .expect("write referenced tsconfig");
        std::fs::write(
            dir.path().join("types").join("root.d.ts"),
            "declare interface RootGlobalProps { root: string }",
        )
        .expect("write root global");
        std::fs::write(
            dir.path()
                .join("types")
                .join("nested")
                .join("included.d.ts"),
            "declare interface IncludedGlobalProps { included?: number }",
        )
        .expect("write included global");
        std::fs::write(
            dir.path().join("types").join("base.d.ts"),
            "declare interface BaseGlobalProps { base: boolean }",
        )
        .expect("write base global");
        std::fs::write(
            dir.path().join("types").join("ref.d.ts"),
            "declare type RefGlobalModel = boolean | string",
        )
        .expect("write referenced global");
        std::fs::write(
            dir.path().join("src").join("ignored.d.ts"),
            "declare interface IgnoredByVueInclude { ignored: string }",
        )
        .expect("write ignored global");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        let discovered = vue3_tsconfig_global_type_files(&filename_text, &type_resolver)
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        let expected_discovered = [
            dir.path().join("types").join("base.d.ts"),
            dir.path().join("types").join("root.d.ts"),
            dir.path()
                .join("types")
                .join("nested")
                .join("included.d.ts"),
            dir.path().join("types").join("ref.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(discovered, expected_discovered);

        let source = r#"<script setup lang="ts">
defineProps<RootGlobalProps & IncludedGlobalProps & BaseGlobalProps>()
defineModel<RefGlobalModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("root: { type: String, required: true }"));
        assert!(script
            .content
            .contains("included: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("base: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [
            dir.path().join("types").join("root.d.ts"),
            dir.path()
                .join("types")
                .join("nested")
                .join("included.d.ts"),
            dir.path().join("types").join("base.d.ts"),
            dir.path().join("types").join("ref.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script
            .content
            .contains("ignored: { type: String, required: true }"));
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("ignored") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_discovers_tsconfig_types_and_type_roots_global_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(dir.path().join("typings").join("chosen"))
            .expect("create chosen type root");
        std::fs::create_dir_all(dir.path().join("typings").join("@scope").join("tool"))
            .expect("create scoped type root");
        std::fs::create_dir_all(dir.path().join("typings").join("ignored"))
            .expect("create ignored type root");
        std::fs::create_dir_all(dir.path().join("base-types").join("base-root"))
            .expect("create base type root");
        std::fs::create_dir_all(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted"),
        )
        .expect("create default @types root");
        std::fs::create_dir_all(dir.path().join("config")).expect("create config dir");
        std::fs::create_dir_all(dir.path().join("project")).expect("create project dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "extends": "./config/base.json",
                "compilerOptions": {
                    "types": ["chosen", "@scope/tool"],
                    "typeRoots": ["./typings"]
                },
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.path().join("config").join("base.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["${configDir}/base-types"]
                }
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(dir.path().join("project").join("tsconfig.json"), "{}")
            .expect("write referenced tsconfig");
        std::fs::write(
            dir.path().join("typings").join("chosen").join("index.d.ts"),
            "declare interface ChosenGlobalProps { chosen: string }",
        )
        .expect("write chosen global");
        std::fs::write(
            dir.path()
                .join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            "declare type ScopedGlobalModel = number | boolean",
        )
        .expect("write scoped global");
        std::fs::write(
            dir.path()
                .join("typings")
                .join("ignored")
                .join("index.d.ts"),
            "declare interface IgnoredTypeRootGlobalProps { ignored: string }",
        )
        .expect("write ignored type root");
        std::fs::write(
            dir.path()
                .join("base-types")
                .join("base-root")
                .join("index.d.ts"),
            "declare interface BaseRootGlobalProps { baseRoot?: number }",
        )
        .expect("write base root global");
        std::fs::write(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        let discovered = vue3_tsconfig_global_type_files(&filename_text, &type_resolver)
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        let expected_discovered = [
            dir.path()
                .join("base-types")
                .join("base-root")
                .join("index.d.ts"),
            dir.path().join("typings").join("chosen").join("index.d.ts"),
            dir.path()
                .join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| normalize_path_string(&path))
        .collect::<BTreeSet<_>>();
        assert_eq!(discovered, expected_discovered);

        let source = r#"<script setup lang="ts">
defineProps<ChosenGlobalProps & BaseRootGlobalProps & DefaultTypesGlobalProps>()
defineModel<ScopedGlobalModel>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("chosen: { type: String, required: true }"));
        assert!(script
            .content
            .contains("baseRoot: { type: Number, required: false }"));
        assert!(script
            .content
            .contains("defaulted: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Number, Boolean] },"));
        assert!(!script.content.contains("ignored: { type: String"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(deps, expected_discovered);
        assert!(!script
            .deps
            .iter()
            .any(|dep| dep.contains("ignored") || dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_respects_empty_configured_tsconfig_type_roots() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src").join("components"))
            .expect("create component dir");
        std::fs::create_dir_all(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted"),
        )
        .expect("create default @types root");
        std::fs::write(
            dir.path()
                .join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["./missing"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.path().join("src").join("components").join("Comp.vue");
        let filename_text = filename.to_string_lossy();
        let type_resolver = vue3_type_resolver_context_for_filename(&filename_text);
        let discovered = vue3_tsconfig_global_type_files(&filename_text, &type_resolver);
        assert!(discovered.is_empty(), "{:?}", discovered);

        let source = r#"<script setup lang="ts">
defineProps<DefaultTypesGlobalProps>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(
            script.errors.iter().any(|error| error
                .contains("Unresolvable type reference or unsupported built-in utility type")),
            "{:?}",
            script.errors
        );
        assert!(script.deps.is_empty(), "{:?}", script.deps);
        assert!(!script.content.contains("defaulted: { type: Boolean"));
    }

    #[test]
    fn vue3_compile_script_resolves_global_type_re_exports_and_deps() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("base.ts");
        std::fs::write(&base, "export interface Base { age: number }").expect("write base type");
        let types = dir.path().join("types.ts");
        std::fs::write(&types, "export type Name = string").expect("write helper type");
        let foo = dir.path().join("foo.ts");
        std::fs::write(
            &foo,
            concat!(
                "import type { Base } from './base'\n",
                "import type { Name } from './types'\n",
                "export interface Foo extends Base { name: Name }"
            ),
        )
        .expect("write foo type");
        let bar = dir.path().join("bar.ts");
        std::fs::write(&bar, "export interface Bar { bar: boolean }").expect("write bar type");
        let baz = dir.path().join("baz.ts");
        std::fs::write(&baz, "export interface Baz { baz: string }").expect("write baz type");
        let package_dir = dir.path().join("node_modules").join("pkg");
        std::fs::create_dir_all(package_dir.join("dist")).expect("create package dir");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write package manifest");
        let package_types = package_dir.join("dist").join("index.d.ts");
        std::fs::write(
            &package_types,
            "export interface PackageType { value: string }",
        )
        .expect("write package types");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare global {\n",
                "  export type { Foo } from './foo'\n",
                "  export { Bar } from './bar'\n",
                "  export * from './baz'\n",
                "  export type { PackageType } from './node_modules/pkg'\n",
                "}\n",
                "export {}\n"
            ),
        )
        .expect("write global re-exports");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<Foo & Bar & Baz & PackageType>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("age: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("name: { type: String, required: true }"));
        assert!(script
            .content
            .contains("bar: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("baz: { type: String, required: true }"));
        assert!(script
            .content
            .contains("value: { type: String, required: true }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global, foo, base, types, bar, baz, package_types]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_resolves_global_declared_extract_prop_types() {
        let dir = tempfile::tempdir().expect("temp dir");
        let global = dir.path().join("global-props.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare const globalProps: {\n",
                "  label: StringConstructor\n",
                "  enabled: { type: BooleanConstructor, required: true }\n",
                "}\n",
                "interface UploadFile<T> { raw: T }\n",
                "declare function uploadProps<T>(): {\n",
                "  fileList: { type: PropType<UploadFile<T>[]>, default: UploadFile<T>[] }\n",
                "}\n"
            ),
        )
        .expect("write global props");

        let filename = dir.path().join("Comp.vue");
        let source = r#"<script setup lang="ts">
defineProps<
  ExtractPropTypes<typeof globalProps> &
  Partial<import('vue').ExtractPropTypes<ReturnType<typeof uploadProps>>>
>()
</script>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("label: { type: String, required: false }"));
        assert!(script
            .content
            .contains("enabled: { type: Boolean, required: true }"));
        assert!(script
            .content
            .contains("fileList: { type: Array, required: false }"));

        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!script.deps.iter().any(|dep| dep.contains('\\')));
    }

    #[test]
    fn vue3_compile_script_global_type_files_use_imports_without_exposing_import_names() {
        let dir = tempfile::tempdir().expect("temp dir");
        let leaf = dir.path().join("leaf.ts");
        std::fs::write(&leaf, "export type ImportedValue = number").expect("write leaf type");
        let global = dir.path().join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "import type { ImportedValue } from './leaf'\n",
                "export {}\n",
                "declare global { interface GlobalProps { imported: ImportedValue; msg: string } }"
            ),
        )
        .expect("write global types");

        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            dir.path().join("Comp.vue").to_string_lossy(),
            r#"<script setup lang="ts">defineProps<GlobalProps>()</script>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("imported: { type: Number, required: true }"));
        assert!(script
            .content
            .contains("msg: { type: String, required: true }"));
        let deps = script.deps.iter().cloned().collect::<BTreeSet<_>>();
        let expected = [global.clone(), leaf]
            .into_iter()
            .map(|path| normalize_path_string(&path))
            .collect::<BTreeSet<_>>();
        assert_eq!(deps, expected);

        let descriptor = compiler.parse(
            dir.path().join("Imported.vue").to_string_lossy(),
            r#"<script setup lang="ts">defineProps<ImportedValue>()</script>"#,
        );
        let imported_script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                global_type_files: vec![global.to_string_lossy().to_string()],
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(
            imported_script.errors.is_empty(),
            "{:?}",
            imported_script.errors
        );
        assert!(
            imported_script.deps.is_empty(),
            "{:?}",
            imported_script.deps
        );
        assert!(!imported_script.content.contains("imported: { type: Number"));
    }

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

    #[test]
    fn compile_wrappers_return_shapes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script lang="ts">export default {}</script><script setup lang="ts">const x = 1</script><style scoped src="./base.css">@import "./dep.css"; .a{ color: v-bind(color); }</style>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
        assert!(template.code.contains("render"));
        assert!(template.ast_summary.starts_with("dom:"));
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 0);
        assert!(script.setup);
        assert_eq!(script.lang.as_deref(), Some("ts"));
        assert_eq!(
            script.bindings.get("x").map(String::as_str),
            Some("setup-const")
        );
        assert!(script.content.contains("_defineComponent"));
        assert!(script.content.contains("__returned__ = { x }"));
        assert_eq!(script.script_ast.len(), 1);
        let script_statement = &script.script_ast[0];
        assert_eq!(script_statement["type"], json!("ExportDefaultDeclaration"));
        assert_eq!(script_statement["start"], json!(0));
        assert_eq!(script_statement["end"], json!("export default {}".len()));
        assert_eq!(script_statement["source"], json!("export default {}"));
        assert_eq!(script_statement["loc"]["start"]["offset"], json!(0));
        assert_eq!(
            script_statement["loc"]["end"]["offset"],
            json!("export default {}".len())
        );
        assert_eq!(
            script_statement["declaration"]["type"],
            json!("ObjectExpression")
        );

        assert_eq!(script.script_setup_ast.len(), 1);
        let setup_statement = &script.script_setup_ast[0];
        assert_eq!(setup_statement["type"], json!("VariableDeclaration"));
        assert_eq!(setup_statement["kind"], json!("const"));
        assert_eq!(setup_statement["source"], json!("const x = 1"));
        assert_eq!(setup_statement["loc"]["start"]["offset"], json!(0));
        assert_eq!(setup_statement["declarations"][0]["id"]["name"], json!("x"));
        assert_eq!(
            setup_statement["declarations"][0]["init"]["value"],
            json!(1.0)
        );
        let script_json = serde_json::to_value(&script).expect("script json");
        assert!(script_json.get("scriptAst").is_some());
        assert!(script_json.get("scriptSetupAst").is_some());
        assert_eq!(
            script_json.get("type").and_then(|value| value.as_str()),
            Some("script")
        );
        let style = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        assert!(style.errors.is_empty());
        assert!(style.map.is_none());
        assert!(style.code.contains("var(--color)"));
        assert_eq!(style.dependencies, vec!["./base.css", "./dep.css"]);
        assert_eq!(style.raw_result.len(), 1);
        let style_json = serde_json::to_value(&style).expect("style json");
        assert!(style_json.get("rawResult").is_some());
    }

    #[test]
    fn compile_style_returns_css_module_exports() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.red { color: red }\n:global(.blue) { color: blue }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(!modules.contains_key("blue"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compile_style_returns_css_modules_values() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value primary: red; @value query: (min-width: 1px); @media query { .button { color: primary; } }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: red"));
    }

    #[test]
    fn compile_style_returns_css_modules_imported_values() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tokens.css"),
            "@value primary: red; .remote { color: primary; }",
        )
        .expect("write dep");
        let filename = dir.path().join("modules.vue");
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value primary, remote as external from "./tokens.css"; .button { composes: external; color: primary; } .external { border-color: primary; }</style>"#;
        let descriptor = compiler.parse(filename.to_string_lossy().to_string(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let external = modules.get("external").expect("external export");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(external.contains("_remote_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_") && value.contains(external)));
        assert!(!result.code.contains("@value"));
        assert!(!result.code.contains("_external_"));
        assert!(!result.code.contains("; }"));
        assert!(result.code.contains("color: red"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn compile_style_returns_css_modules_missing_imported_value_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tokens.css"),
            "@value primary: red; .remote { color: primary; }",
        )
        .expect("write dep");
        let filename = dir.path().join("modules.vue");
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value missing from "./tokens.css"; .button { composes: missing; color: missing; }</style>"#;
        let descriptor = compiler.parse(filename.to_string_lossy().to_string(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty());
        assert_eq!(
            modules.get("missing").map(String::as_str),
            Some("undefined")
        );
        assert!(button.contains("_button_"));
        assert!(button.contains("undefined"));
        assert!(!button.contains("i__const_missing_0"));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("color: i__const_missing_0"));
    }

    #[test]
    fn compile_style_forwards_css_modules_dashes_convention() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.foo-bar { color: red }\n.foo_bar { color: blue }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("_foo-bar_")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
    }

    #[test]
    fn compile_style_forwards_css_modules_hash_prefix() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.button { color: red }</style>"#;
        let descriptor = compiler.parse("src/Comp.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    generate_scoped_name: Some("[local]__[hash:base64:5]".into()),
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("button__2G66Z")
        );
        assert!(result.code.contains(".button__2G66Z"));
    }

    #[test]
    fn compile_style_forwards_css_modules_global_module_paths() {
        let mut compiler = SfcCompiler::new();
        let source =
            r#"<style module>.button { color: red }:local(.forced) { color: blue }</style>"#;
        let descriptor = compiler.parse("src/theme.global.css", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(!modules.contains_key("button"));
        assert!(modules
            .get("forced")
            .is_some_and(|value| value.contains("_forced_")));
        assert!(result.code.contains(".button { color: red }"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compile_style_returns_css_modules_id_exports() {
        let source = r#"<style module>#panel { color: red }.button#item { color: blue }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Selectors.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_7jaos_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_7jaos_1")
        );
        assert_eq!(
            modules.get("item").map(String::as_str),
            Some("_item_7jaos_1")
        );
        assert!(result.code.contains("#_panel_7jaos_1"));
        assert!(result.code.contains("._button_7jaos_1#_item_7jaos_1"));
    }

    #[test]
    fn compile_style_leaves_css_modules_class_attribute_selectors_global() {
        let source = r#"<style module>[class="btn"] { color: red }:local([class='forced']) { color: blue }.btn { color: black }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Attr.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("btn")
            .is_some_and(|value| value.contains("_btn_")));
        assert!(!modules.contains_key("forced"));
        assert!(result.code.contains("[class=\"btn\"] { color: red }"));
        assert!(result.code.contains("[class='forced'] { color: blue }"));
        assert!(result.code.contains("._btn_"));
    }

    #[test]
    fn compile_style_returns_css_modules_keyframe_exports() {
        let source = r#"<style module>@keyframes fade { from { opacity: 0 } to { opacity: 1 } }
.button { animation-name: fade; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Anim.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_17sru_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_17sru_2")
        );
        assert!(result.code.contains("@keyframes _fade_17sru_1"));
        assert!(result.code.contains("animation-name: _fade_17sru_1"));
    }

    #[test]
    fn compile_style_forwards_css_modules_export_globals() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.local :global(.global) { color: red }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("local")
            .is_some_and(|value| value.contains("_local_")));
        assert_eq!(modules.get("global").map(String::as_str), Some("global"));
    }

    #[test]
    fn compile_style_returns_css_modules_composes_exports() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.base { color: blue }.button { composes: base global(extra); color: red }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(button.contains(base));
        assert!(button.contains("extra"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_icss_exports() {
        let mut compiler = SfcCompiler::new();
        let source =
            r#"<style module>:export { primary: red; }.button { color: primary; }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compile_style_rewrites_css_modules_icss_import_symbols() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        std::fs::write(
            dir.path().join("dep.css"),
            ".dep { color: blue; }\n:export { token: green; query: (min-width: 1px); }",
        )
        .expect("write dep");
        let source = r#"<style module>:import("./dep.css") { imported: dep; shade: token; mq: query; }.shade { color: shade; }.imported { color: shade; }@media mq { .button { color: shade; } }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.contains(".green"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compile_style_preserves_empty_css_modules_for_missing_icss_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        std::fs::write(dir.path().join("dep.css"), ":export { token: green; }").expect("write dep");
        let source = r#"<style module>:import("./dep.css") { shade: missing; }.shade { color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("empty css modules map");

        assert!(modules.is_empty());
        assert!(result.errors.is_empty());
        assert!(!result.code.contains(":import"));
        assert!(result.code.contains(".shade { color: red"));
        assert!(!result.code.contains("_shade_"));
    }

    #[test]
    fn compile_style_rewrites_css_modules_native_nested_rules() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let source = r#"<style module>.foo { color: blue; .bar { color: red; } &.active { color: green; } @media (min-width: 1px) { :global(.global) { color: black; } :local(.inner) { color: white; } } color: yellow; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        for key in ["foo", "bar", "active", "inner"] {
            assert!(
                modules.get(key).is_some_and(|value| value.contains('_')),
                "missing module key {key}: {modules:?}"
            );
        }
        assert!(!modules.contains_key("global"));
        assert!(result.code.contains("{ color: blue;\n"));
        assert!(result.code.contains("\n._bar_"));
        assert!(result.code.contains("\n&._active_"));
        assert!(result.code.contains("@media (min-width: 1px) {\n.global"));
        assert!(result.code.contains("\n._inner_"));
        assert!(result.code.contains("} color: yellow;"));
        assert!(!result.code.contains("\n.bar {"));
        assert!(!result.code.contains("\n&.active {"));
        assert!(!result.code.contains(":local(.inner)"));
        assert!(!result.code.contains(":global(.global)"));
    }

    #[test]
    fn compile_style_reports_css_modules_native_nested_composes() {
        let source = r#"<style module>.foo { .bar { composes: foo; color: red; } }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(
            result.errors,
            vec![
                "composition is not allowed in nested rule \n\n:local(.bar) { composes: foo; color: red;\n}"
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn compile_style_returns_css_modules_external_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");
        let source =
            r#"<style module>.button { composes: dep from "./dep.css"; color: token; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_node_modules_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(dist_dir.join("theme.css"), ".dep { color: blue; }").expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":"./dist/theme.css"}}"#,
        )
        .expect("write package");
        let source = r#"<style module>.button { composes: dep from "vuec-css-fixture/theme.css"; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_composes_from_global() {
        let source =
            r#"<style module>.button { composes: reset utility from global; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("reset"));
        assert!(button.contains("utility"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_maps_css_modules_composes_diagnostics_to_vue_source() {
        let source = r#"<template></template>
<style module>.button { composes: missing; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let missing_start = source.find("missing").expect("missing token");

        assert_eq!(
            result.errors,
            vec!["referenced class name \"missing\" in composes not found"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert_eq!(
            result.diagnostics[0].span,
            Some(vuec_source::Span::new(
                descriptor.source_file,
                missing_start,
                missing_start + "missing".len()
            ))
        );
    }

    #[test]
    fn compile_style_reports_css_modules_complex_composes_selector() {
        let source =
            r#"<style module>.button.extra { composes: base; }.base { color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(
            result.errors,
            vec![
                "composition is only allowed when selector is single :local class name not in \":local(.button):local(.extra)\""
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_forwards_scss_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("import.scss");
        std::fs::write(&import, ".imported { color: $red; }\n").expect("write import");
        let source = r#"<style lang="scss">
@import "./import.scss";
.square { @include square(10px); }
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some(
                        "$red: red;\n@mixin square($size) { width: $size; height: $size; }".into(),
                    ),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("width: 10px;"));
        let mut resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .to_string();
        if let Some(stripped) = resolved_import.strip_prefix(r"\\?\") {
            resolved_import = stripped.to_string();
        } else if let Some(stripped) = resolved_import.strip_prefix("//?/") {
            resolved_import = stripped.to_string();
        }
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_forwards_less_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("tokens.less");
        std::fs::write(&import, "@space: 6px;\n.imported { margin: @space; }\n")
            .expect("write import");
        let source = r#"<style lang="less">
@import "./tokens.less";
.card {
  color: @brand;
  .title {
    padding: @space;
  }
}
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("@brand: red;".into()),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("margin: 6px;"));
        assert!(result.code.contains(".card .title"));
        assert!(result.code.contains("padding: 6px;"));
        assert!(result.code.contains("color: red;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_forwards_stylus_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("tokens.styl");
        std::fs::write(&import, "space = 6px\n.imported\n  margin space\n").expect("write import");
        let source = r#"<style lang="stylus">
@import "./tokens"
.card
  color brand
  .title
    padding space
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("brand = red".into()),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("margin: 6px;"));
        assert!(result.code.contains(".card .title"));
        assert!(result.code.contains("padding: 6px;"));
        assert!(result.code.contains("color: red;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_uses_vue3_css_var_names_by_default() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            "<style>.foo { font-size: v-bind('font.size'); font-weight: v-bind(_φ); }</style>",
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(r"var(--test-font\.size)"));
        assert!(result.code.contains("var(--test-_φ)"));
    }

    #[test]
    fn compile_style_rewrites_comment_separated_css_vars() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style>.foo { color: v-bind /*x*/ (color); font-size: v-bind/**/ ('font.size'); height: v-bind/**/(height); }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("var(--test-color)"));
        assert!(result.code.contains(r"var(--test-font\.size)"));
        assert!(result.code.contains("v-bind/**/(height)"));
    }

    #[test]
    fn compile_style_rewrites_top_level_is_where_scoped_branches() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(.foo, .bar):hover { color: red; }:where(.one .child, .two > .item) { color: blue; }.host:is(.foo, .bar) { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is(.foo[data-v-test], .bar[data-v-test]):hover"));
        assert!(result
            .code
            .contains(":where(.one .child[data-v-test], .two > .item[data-v-test])"));
        assert!(result.code.contains(".host[data-v-test]:is(.foo, .bar)"));
    }

    #[test]
    fn compile_style_rewrites_native_nested_scoped_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.foo { color: blue; .bar { color: red; } @media (min-width: 1px) { &:hover { color: green; } } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".foo {"));
        assert!(result.code.contains("&[data-v-test] { color: blue;"));
        assert!(result.code.contains(".bar[data-v-test] { color: red;"));
        assert!(result.code.contains("@media (min-width: 1px) {"));
        assert!(result.code.contains("&[data-v-test]:hover { color: green;"));
    }

    #[test]
    fn compile_style_rewrites_direct_nested_parent_selectors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>*.foo { color: blue; .bar { color: red; } }.foo /*x*/ .bar { .child { color: orange; } }:is(.foo /*x*/ .bar, *.baz) { .child { color: purple; } }:is(:global(.g), :slotted(.s), * .item):hover { color: green; .child { color: yellow; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".foo {"));
        assert!(!result.code.contains("*.foo {"));
        assert!(result.code.contains(":is(.g,.s,.item):hover {"));
        assert!(result.code.contains(".foo /*x*/ .bar {"));
        assert!(result.code.contains(":is(.foo  .bar,.baz) {"));
        assert!(result.code.contains(".bar[data-v-test] { color: red;"));
        assert!(result.code.contains(".child[data-v-test] { color: yellow;"));
    }

    #[test]
    fn compile_style_rewrites_first_normal_deep_container_nested_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(.foo, :deep(.bar), .baz) { color: blue; .child { color: red; } }.host :where(:global(.g), :slotted(.s), :deep(.d), .tail) { color: green; & .child { color: yellow; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is(.foo,[data-v-test] .bar, .baz[data-v-test])[data-v-test] {"));
        assert!(result.code.contains("& { color: blue;"));
        assert!(result
            .code
            .contains(".host[data-v-test] :where(.g,.s,[data-v-test] .d, .tail[data-v-test]) {"));
        assert!(result.code.contains("& .child { color: yellow;"));
    }

    #[test]
    fn compile_style_rewrites_first_normal_deep_container_suffix_nested_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(.foo, :deep(.bar), .baz):hover { color: blue; .child { color: red; } }.host :where(.foo, :deep(.bar), :global(.g), :slotted(.s)):hover { color: green; .child { color: yellow; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(
            ":is(.foo):hover, :is([data-v-test] .bar)[data-v-test]:hover, :is(.baz[data-v-test]):hover {"
        ));
        assert!(result.code.contains("& { color: blue;"));
        assert!(result.code.contains(
            ".host[data-v-test] :where(.foo,[data-v-test] .bar,.g,[data-v-test].s[data-v-test-s]):hover {"
        ));
        assert!(result.code.contains(".child { color: yellow;"));
    }

    #[test]
    fn compile_style_rewrites_deep_nested_scoped_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:deep(.foo, .bar) { color: blue; .child { color: red; } @media (min-width: 1px) { .inner { color: green; } } }:deep(.anchor) { color: blue; & .child { color: red; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("[data-v-test] .foo {"));
        assert!(result.code.contains("color: blue;"));
        assert!(result.code.contains(".child { color: red;"));
        assert!(result.code.contains("@media (min-width: 1px) {"));
        assert!(result.code.contains(".inner { color: green;"));
        assert!(result
            .code
            .contains("[data-v-test] .anchor { color: blue;\n& .child { color: red;"));
        assert!(!result.code.contains(".bar"));
        assert!(!result.code.contains(".child[data-v-test]"));
        assert!(!result.code.contains(".inner[data-v-test]"));
    }

    #[test]
    fn compile_style_rewrites_slotted_universal_combinators() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:slotted(* + .foo) { color: red; }:is(:slotted(* + .bar), .baz) { color: blue; }:slotted(:is(.alpha, .beta)) { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("+ .foo[data-v-test-s]"));
        assert!(result
            .code
            .contains(":is(+ .bar[data-v-test-s], .baz[data-v-test])"));
        assert!(result
            .code
            .contains(":is(.alpha[data-v-test-s], .beta[data-v-test-s])"));
    }

    #[test]
    fn compile_style_preserves_scoped_selector_list_spacing() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.a,.b { color: red; }.a, :slotted(.b) { color: blue; }.a, :where(.b).active { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".a[data-v-test],.b[data-v-test]"));
        assert!(result.code.contains(".a[data-v-test],.b[data-v-test-s]"));
        assert!(result
            .code
            .contains(".a[data-v-test], :where(.b).active[data-v-test]"));
    }

    #[test]
    fn compile_style_rewrites_escaped_scoped_selector_tokens() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.foo\:bar { color: red; }.foo\,bar { color: blue; }:slotted(.foo\:bar) { color: green; }.foo\:deep(.bar) { color: yellow; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(r#".foo\:bar[data-v-test]"#));
        assert!(result.code.contains(r#".foo\,bar[data-v-test]"#));
        assert!(result.code.contains(r#".foo\:bar[data-v-test-s]"#));
        assert!(result.code.contains(r#".foo\:deep(.bar)[data-v-test]"#));
    }

    #[test]
    fn compile_style_rewrites_commented_scoped_selectors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.foo/*,*/.bar { color: red; }.foo /*x*/:hover { color: blue; }:is(.foo/*:deep(.bar)*/.baz, .qux) { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".foo/*,*/.bar[data-v-test]"));
        assert!(result.code.contains(".foo[data-v-test] :hover"));
        assert!(result
            .code
            .contains(":is(.foo/*:deep(.bar)*/.baz[data-v-test], .qux[data-v-test])"));
    }

    #[test]
    fn compile_style_rewrites_deep_container_special_branches() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(:slotted(.foo), :deep(.bar), :global(.baz), .qux) { color: red; }.host:is(:deep(.foo), :global(.bar), :slotted(.baz), .qux) { @media (min-width:1px){ .child { color:red; } } }:is(:deep(.foo), :global(.bar), :slotted(.baz), .qux) { color: blue; .child { color:red; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is(.foo[data-v-test-s],[data-v-test] .bar,.baz, .qux[data-v-test])"));
        assert!(result
            .code
            .contains(".host[data-v-test]:is( .foo,.bar,.baz, .qux)"));
        assert!(result.code.contains(
            ":is([data-v-test] .foo,.bar,[data-v-test].baz[data-v-test-s], .qux[data-v-test])[data-v-test]"
        ));
    }

    #[test]
    fn compile_style_rewrites_deep_container_split_pseudo_suffix() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(:deep(.d), .n):hover { color:red; }:where(.x :deep(.d), :slotted(.s))::before { color:red; }:has(.n,:deep(.d),.m):hover { color:red; }:where(:deep(.d), :slotted(.s))::before { color: blue; .child { color: red; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is([data-v-test] .d):hover, :is(.n[data-v-test]):hover"));
        assert!(result
            .code
            .contains(":where(.x[data-v-test] .d)::before, :where(.s[data-v-test-s])::before"));
        assert!(result.code.contains(
            "[data-v-test]:has(.n):hover, :has([data-v-test] .d):hover,[data-v-test]:has(.m):hover"
        ));
        assert!(result.code.contains(
            ":where([data-v-test] .d)[data-v-test]::before, :where([data-v-test].s[data-v-test-s])::before"
        ));
    }

    #[test]
    fn compile_style_rewrites_deep_passthrough_nested_at_rule_special_selectors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:deep(.d) { @media (min-width:1px){ :deep(.inner) { color:red; } :global(.g) { color:blue; } :slotted(.s) { color:green; } } }:is(:deep(.d), .n) { color: blue; @media (min-width:1px){ .x :deep(.inner) { color:red; } } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("[data-v-test] .d {"));
        assert!(result.code.contains(" .inner { color:red;"));
        assert!(result.code.contains(".g { color:blue;"));
        assert!(result.code.contains(".s { color:green;"));
        assert!(result
            .code
            .contains(":is([data-v-test] .d, .n[data-v-test]) { color: blue;"));
        assert!(result.code.contains(".x .inner { color:red;"));
        assert!(!result.code.contains(":deep(.inner)"));
        assert!(!result.code.contains(":global(.g)"));
        assert!(!result.code.contains(":slotted(.s)"));
    }

    #[test]
    fn compile_style_emits_vue3_deprecated_deep_warnings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>>>> .foo { color: red; } ::v-deep .bar { color: blue; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR"
                && diagnostic.severity == Severity::Warning
        }));
        assert!(result.diagnostics[0]
            .message
            .contains("the >>> and /deep/ combinators have been deprecated"));
        assert!(result.diagnostics[1]
            .message
            .contains("::v-deep usage as a combinator has been deprecated"));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn compile_style_source_map_merges_style_blocks_to_vue_source() {
        let mut compiler = SfcCompiler::new();
        let source = "<style>.a { color: red; }</style>\n<style>.b { color: blue; }</style>";
        let descriptor = compiler.parse("multi.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                source_map: true,
                ..SfcStyleCompileOptions::default()
            },
        );
        let map = result.map.expect("merged style source map");

        assert_eq!(map.sources, vec!["multi.vue"]);
        assert_eq!(
            map.sources_content
                .as_ref()
                .and_then(|sources| sources[0].as_ref()),
            Some(&source.to_string())
        );
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("first style mapping");
        assert_eq!(first.source, "multi.vue");
        assert_eq!(first.line, 0);
        assert_eq!(first.column, "<style>".len() as u32);
        let second_generated_line = result.code.lines().count().saturating_sub(1) as u32;
        let second = map
            .original_position(vuec_source::GeneratedPosition::new(
                second_generated_line,
                0,
            ))
            .unwrap()
            .expect("second style mapping");
        assert_eq!(second.source, "multi.vue");
        assert_eq!(second.line, 1);
        assert_eq!(second.column, "<style>".len() as u32);
    }

    #[test]
    fn compile_style_source_map_skips_empty_style_blocks() {
        let mut compiler = SfcCompiler::new();
        let source = "<style></style>\n<style>.b { color: blue; }</style>";
        let descriptor = compiler.parse("multi.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                source_map: true,
                ..SfcStyleCompileOptions::default()
            },
        );
        let map = result.map.expect("merged style source map");

        assert_eq!(result.code, ".b { color: blue;\n}");
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("non-empty style mapping");
        assert_eq!(first.source, "multi.vue");
        assert_eq!(first.line, 1);
        assert_eq!(first.column, "<style>".len() as u32);
    }

    #[test]
    fn compile_style_preserves_plain_css_imports_without_resolve_diagnostics() {
        let mut compiler = SfcCompiler::new();
        let source = "<template><div/></template>\n<style>\n.a { color: red; }\n@import \"./not-missing.css\";\n@import \"missing.css\";\n</style>";
        let descriptor = compiler.parse("diagnostic.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert!(result.errors.is_empty());
        assert!(result.diagnostics.is_empty());
        assert!(result.code.contains("@import \"./not-missing.css\";"));
        assert!(result.code.contains("@import \"missing.css\";"));
    }

    #[test]
    fn compile_template_uses_ssr_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("foo.vue", r#"<template><div>{{ msg }}</div></template>"#);
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );
        assert!(template.code.contains("ssrRender"));
        assert!(template.code.contains("_ssrInterpolate(_ctx.msg)"));
    }

    #[test]
    fn compile_template_passes_asset_url_base_to_dom_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png"><img src="~logo.png"><img srcset="@/logo.png 1x, ./logo.png 2x"></template>"#,
        );
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains(r#"src: "/foo/logo.png""#));
        assert!(template.code.contains("import _imports_0 from 'logo.png'"));
        assert!(template
            .code
            .contains("import _imports_1 from '@/logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template
            .code
            .contains(r#"const _hoisted_1 = _imports_1 + ' 1x, ' + "/foo/logo.png" + ' 2x'"#));
        assert!(template.code.contains("srcset: _hoisted_1"));
        assert!(!template.code.contains(r#"src: "~logo.png""#));
    }

    #[test]
    fn compile_template_supports_custom_asset_url_tags() {
        let mut compiler = SfcCompiler::new();
        let descriptor =
            compiler.parse("foo.vue", r#"<template><foo bar="~baz"></foo></template>"#);
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                asset_url_options: AssetUrlOptions {
                    tags,
                    ..AssetUrlOptions::default()
                },
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains("import _imports_0 from 'baz'"));
        assert!(template.code.contains("bar: _imports_0"));
    }

    #[test]
    fn compile_template_transforms_asset_urls_to_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png" srcset="./logo.png 2x"><img src="@theme/logo.png"></template>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());

        assert!(template
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(template
            .code
            .contains("import _imports_1 from '@theme/logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template
            .code
            .contains("const _hoisted_1 = _imports_0 + ' 2x'"));
        assert!(template.code.contains("srcset: _hoisted_1"));
        assert!(!template.code.contains("_ctx._imports_"));
        assert!(!template.code.contains("PROPS"));
    }

    #[test]
    fn compile_template_honors_hoist_static_option() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div><img src="./logo.png"><span>ok</span></div></template>"#,
        );
        let hoisted = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
        let unhoisted = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                hoist_static: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(hoisted.code.contains("_cache[0]"));
        assert!(hoisted.code.contains("src: _imports_0"));
        assert!(!unhoisted.code.contains("_cache[0]"));
        assert!(unhoisted.code.contains("src: _imports_0"));
    }

    #[test]
    fn compile_template_uses_official_cache_handler_default() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><input @blur="onBlur" @[validateEvent]="onValidateEvent"></template>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());

        assert!(template.code.contains("toHandlerKey as _toHandlerKey"));
        assert!(template.code.contains("mergeProps as _mergeProps"));
        assert!(template.code.contains(
            "_cache[0] || (_cache[0] = (...args) => (_ctx.onBlur && _ctx.onBlur(...args)))"
        ));
        assert!(template.code.contains("_cache[1] || (_cache[1] = (...args) => (_ctx.onValidateEvent && _ctx.onValidateEvent(...args)))"));
        assert!(!template.code.contains("data-vuec-dom"));
    }

    #[test]
    fn compile_template_source_does_not_cache_dynamic_interpolation_subtrees() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "contract.vue",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            SfcTemplateCompileOptions {
                scope_id: Some("data-v-contract".into()),
                slotted: false,
                ssr: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains("_toDisplayString(_ctx.msg)"));
        assert!(template.code.contains("1 /* TEXT */"));
        assert!(!template.code.contains("-1 /* CACHED */"));
        assert!(!template.code.contains("[...(_cache[0]"));
        assert_eq!(template.errors.len(), 2);
        assert_eq!(template.errors[0].code, 64);
        assert_eq!(template.errors[1].code, 64);
    }

    #[test]
    fn compile_template_source_returns_dom_compile_errors() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "x.vue",
            r#"<div :bar="a[" v-model="baz"/>"#,
            SfcTemplateCompileOptions::default(),
        );

        assert_eq!(template.errors.len(), 2);
        assert_eq!(template.errors[0].code, 46);
        assert_eq!(template.errors[0].loc.start.offset, 13);
        assert_eq!(template.errors[1].code, 58);
        assert_eq!(template.errors[1].loc.source, r#"v-model="baz""#);
    }

    #[test]
    fn vue27_template_preprocessor_compiles_pug_and_reports_missing_lang() {
        let compiler = SfcCompiler::new();
        let pug = compiler.preprocess_vue27_template(
            "body\n h1 Pug Examples\n div.container\n   p Cool Pug example!\n",
            Vue27TemplatePreprocessOptions {
                lang: Some("pug".into()),
                filename: Some("example.vue".into()),
            },
        );

        assert!(pug.errors.is_empty());
        assert_eq!(
            pug.source,
            r#"<body><h1>Pug Examples</h1><div class="container"><p>Cool Pug example!</p></div></body>"#
        );

        let missing = compiler.preprocess_vue27_template(
            "",
            Vue27TemplatePreprocessOptions {
                lang: Some("unknownLang".into()),
                filename: Some("example.vue".into()),
            },
        );
        assert_eq!(missing.errors.len(), 1);
        assert_eq!(missing.tips.len(), 1);
        assert!(missing.errors[0].contains("however it is not installed"));
        assert!(missing.tips[0].contains("Please install"));
    }

    #[test]
    fn compile_template_ssr_transforms_asset_urls_to_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png" srcset="./logo.png 2x"></template>"#,
        );
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(template.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!template.code.contains("</img>"));
        assert!(!template.code.contains("_ctx._imports_"));
    }

    #[test]
    fn compile_template_source_ssr_respects_disabled_asset_url_transform() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "foo.vue",
            r#"<img src="./logo.png">"#,
            SfcTemplateCompileOptions {
                ssr: true,
                transform_asset_urls: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(!template.code.contains("import _imports_0"));
        assert!(template.code.contains(r#"src: "./logo.png""#));
        assert!(template.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!template.code.contains("</img>"));
    }
}
