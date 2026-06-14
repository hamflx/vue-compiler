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
