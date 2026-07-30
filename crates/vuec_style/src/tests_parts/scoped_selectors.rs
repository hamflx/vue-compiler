    use crate::*;

    #[test]
    fn rewrites_scoped_selectors() {
        let code = rewrite_scoped_selectors(".a, .b { color: red; }", "data-v-x");
        assert!(code.contains(".a[data-v-x]"));
        assert!(code.contains(".b[data-v-x]"));
    }

    #[test]
    fn compile_style_matches_official_selector_brace_spacing() {
        let result = compile_style(
            ".a{ color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-contract".into()),
                scoped: true,
                ..StyleCompileOptions::default()
            },
        );
        assert_eq!(
            result.code,
            ".a[data-v-contract]{ color: var(--contract-color);\n}"
        );

        let spaced = compile_style(
            ".a { color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-contract".into()),
                scoped: true,
                ..StyleCompileOptions::default()
            },
        );
        assert_eq!(
            spaced.code,
            ".a[data-v-contract] { color: var(--contract-color);\n}"
        );
    }

    #[test]
    fn compile_style_matches_vue3_official_scoped_nested_output() {
        let options = StyleCompileOptions {
            id: Some("data-v-test".into()),
            scoped: true,
            ..StyleCompileOptions::default()
        };

        let direct_nested = compile_style(
            "main {\n  width: 100%;\n  > * {\n    max-width: 200px;\n  }\n}",
            options.clone(),
        );
        assert_eq!(
            direct_nested.code,
            "main {\n&[data-v-test] {\n  width: 100%;\n}\n> *[data-v-test] {\n    max-width: 200px;\n}\n}"
        );

        let nested_at_rule = compile_style(
            "h1 {\ncolor: red;\n/*background-color: pink;*/\n@media only screen and (max-width: 800px) {\n  background-color: green;\n  .bar { color: white }\n}\n.foo { color: red; }\n}",
            options.clone(),
        );
        assert_eq!(
            nested_at_rule.code,
            "h1 {\n&[data-v-test] {\ncolor: red\n/*background-color: pink;*/\n}\n@media only screen and (max-width: 800px) {\n&[data-v-test] {\n  background-color: green\n}\n.bar[data-v-test] { color: white\n}\n}\n.foo[data-v-test] { color: red;\n}\n}"
        );

        let media = compile_style("@media print { .foo { color: red }}", options.clone());
        assert_eq!(
            media.code,
            "@media print {\n.foo[data-v-test] { color: red\n}}"
        );

        let supports = compile_style(
            "@supports(display: grid) { .foo { display: grid }}",
            options,
        );
        assert_eq!(
            supports.code,
            "@supports(display: grid) {\n.foo[data-v-test] { display: grid\n}}"
        );
    }

    #[test]
    fn rewrites_vue27_scoped_deep_pseudo_and_keyframes() {
        let code = rewrite_scoped_selectors(
            r#"
.foo p >>> .bar { color: red; }
::selection { display: none; }
.test:after { content: 'bye!'; }
@keyframes color { from { color: red; } to { color: green; } }
.anim { animation: color 5s infinite, other 5s; }
.names { animation-name: color, other; }
"#,
            "v-scope-xxx",
        );

        assert!(code.contains(".foo p[v-scope-xxx] .bar { color: red;"));
        assert!(code.contains("[v-scope-xxx]::selection { display: none;"));
        assert!(code.contains(".test[v-scope-xxx]:after { content: 'bye!';"));
        assert!(code.contains("@keyframes color-v-scope-xxx {"));
        assert!(code.contains("animation: color-v-scope-xxx 5s infinite, other 5s;"));
        assert!(code.contains("animation-name: color-v-scope-xxx,other;"));
    }

    #[test]
    fn rewrites_scoped_slotted_selectors_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":slotted(.foo) { color: red; }", "data-v-test"),
            ".foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".baz .qux ::v-slotted(.foo .bar) { color: red; }",
                "data-v-test",
            ),
            ".baz .qux .foo .bar[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":slotted(.foo):hover { color: red; }", "data-v-test"),
            ".foo[data-v-test-s]:hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".wrapper:slotted(.foo) { color: red; }", "data-v-test"),
            ".wrapper.foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(.foo) .bar { color: red; }", "data-v-test"),
            ".a .foo[data-v-test-s] .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(*:hover) { color: red; }", "data-v-test"),
            ".a [data-v-test-s]:hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(*.foo) { color: red; }", "data-v-test"),
            ".a .foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(* + .foo) { color: red; }", "data-v-test"),
            ".a  + .foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":slotted(* + .foo) { color: red; }", "data-v-test"),
            "+ .foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(:slotted(* + .foo), .bar) { color: red; }",
                "data-v-test",
            ),
            ":is(+ .foo[data-v-test-s], .bar[data-v-test]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":where(:slotted(* + .foo)) { color: red; }", "data-v-test",),
            ":where(+ .foo[data-v-test-s]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(:slotted(* + .foo), :slotted(* ~ .bar)) { color: red; }",
                "data-v-test",
            ),
            ":is(+ .foo[data-v-test-s],~ .bar[data-v-test-s]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":slotted(:is(.foo, .bar)) { color: red; }", "data-v-test",),
            ":is(.foo[data-v-test-s], .bar[data-v-test-s]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":slotted(:where(.foo, .bar)) { color: red; }",
                "data-v-test",
            ),
            ":where(.foo[data-v-test-s], .bar[data-v-test-s]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".host :slotted(:is(.foo, .bar)) { color: red; }",
                "data-v-test",
            ),
            ".host :is(.foo[data-v-test-s], .bar[data-v-test-s]) { color: red; }"
        );
    }

    #[test]
    fn rewrites_top_level_global_selectors_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":global(.foo) { color: red; }", "data-v-test"),
            ".foo { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors("::v-global(.foo .bar) { color: red; }", "data-v-test"),
            ".foo .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".baz .qux ::v-global(.foo .bar) { color: red; }",
                "data-v-test",
            ),
            ".foo .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :global(.b) .c { color: red; }", "data-v-test"),
            ".b { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":global(.foo, .bar) { color: red; }", "data-v-test"),
            ".foo { color: red; }"
        );
    }

    #[test]
    fn leaves_nested_global_pseudo_scoped_on_outer_selector() {
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(:global(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":is(:global(.foo), .bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":where(:global(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":where(:global(.foo), .bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":not(:global(.foo)) .bar { color: red; }", "data-v-test"),
            ":not(:global(.foo)) .bar[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":has(:global(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":has(:global(.foo), .bar) .baz[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_top_level_is_where_branches_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo, .bar) { color: red; }", "data-v-test"),
            ":is(.foo[data-v-test], .bar[data-v-test]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo,.bar) { color: red; }", "data-v-test"),
            ":is(.foo[data-v-test],.bar[data-v-test]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":where(.foo .child, .bar > .item) { color: red; }",
                "data-v-test",
            ),
            ":where(.foo .child[data-v-test], .bar > .item[data-v-test]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo, .bar):hover { color: red; }", "data-v-test"),
            ":is(.foo[data-v-test], .bar[data-v-test]):hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":where(.foo, :is(.bar, .baz)) { color: red; }",
                "data-v-test",
            ),
            ":where(.foo[data-v-test], :is(.bar[data-v-test], .baz[data-v-test])) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(:global(.foo), .bar) { color: red; }", "data-v-test",),
            ":is(.foo, .bar[data-v-test]) { color: red; }"
        );
    }

    #[test]
    fn leaves_non_target_is_where_pseudos_on_outer_scoped_selector() {
        assert_eq!(
            rewrite_scoped_selectors(".host:is(.foo, .bar) { color: red; }", "data-v-test"),
            ".host[data-v-test]:is(.foo, .bar) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".host :where(.foo, .bar) { color: red; }", "data-v-test",),
            ".host[data-v-test] :where(.foo, .bar) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":where(.foo, .bar).active { color: red; }", "data-v-test",),
            ":where(.foo, .bar).active[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo) [x] { color: red; }", "data-v-test"),
            ":is(.foo) [x][data-v-test] { color: red; }"
        );
    }

    #[test]
    fn preserves_scoped_selector_list_spacing_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(".a,.b { color: red; }", "data-v-test"),
            ".a[data-v-test],.b[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a,    .b { color: red; }", "data-v-test"),
            ".a[data-v-test],    .b[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a, :slotted(.b) { color: red; }", "data-v-test"),
            ".a[data-v-test],.b[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a, :deep(.b) { color: red; }", "data-v-test"),
            ".a[data-v-test],[data-v-test] .b { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a, :global(.b) { color: red; }", "data-v-test"),
            ".a[data-v-test],.b { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a, :not(.b) { color: red; }", "data-v-test"),
            ".a[data-v-test],[data-v-test]:not(.b) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a, :where(.b).active { color: red; }", "data-v-test"),
            ".a[data-v-test], :where(.b).active[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a, * .b { color: red; }", "data-v-test"),
            ".a[data-v-test],.b[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_escaped_scoped_selector_tokens_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(r#".foo\:bar { color: red; }"#, "data-v-test"),
            r#".foo\:bar[data-v-test] { color: red; }"#
        );
        assert_eq!(
            rewrite_scoped_selectors(r#".foo\,bar { color: red; }"#, "data-v-test"),
            r#".foo\,bar[data-v-test] { color: red; }"#
        );
        assert_eq!(
            rewrite_scoped_selectors(r#".foo\:global(.bar) { color: red; }"#, "data-v-test"),
            r#".foo\:global(.bar)[data-v-test] { color: red; }"#
        );
        assert_eq!(
            rewrite_scoped_selectors(r#":is(.foo\:bar, .baz) { color: red; }"#, "data-v-test"),
            r#":is(.foo\:bar[data-v-test], .baz[data-v-test]) { color: red; }"#
        );
        assert_eq!(
            rewrite_scoped_selectors(r#":slotted(.foo\:bar) { color: red; }"#, "data-v-test"),
            r#".foo\:bar[data-v-test-s] { color: red; }"#
        );
        assert_eq!(
            rewrite_scoped_selectors(r#".\31 foo:hover { color: red; }"#, "data-v-test"),
            r#".\31 foo[data-v-test]:hover { color: red; }"#
        );
        assert_eq!(
            rewrite_scoped_selectors(".你好 { color: red; }", "data-v-test"),
            ".你好[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(r#"* .foo\:bar { color: red; }"#, "data-v-test"),
            r#".foo\:bar[data-v-test] { color: red; }"#
        );
    }

    #[test]
    fn escaped_deep_like_tokens_do_not_emit_deprecated_warnings() {
        let result = compile_style(
            r#".foo\:deep(.bar) { color: red; }"#,
            StyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                warn_deprecated_scoped_selectors: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.diagnostics.is_empty());
        assert_eq!(
            result.code,
            ".foo\\:deep(.bar)[data-v-test] { color: red;\n}"
        );
    }

    #[test]
    fn rewrites_commented_scoped_selectors_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(".foo/*x*/.bar { color: red; }", "data-v-test"),
            ".foo/*x*/.bar[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo /*x*/ .bar { color: red; }", "data-v-test"),
            ".foo  .bar[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo/*,*/.bar { color: red; }", "data-v-test"),
            ".foo/*,*/.bar[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo/*:deep(.bar)*/.baz { color: red; }", "data-v-test"),
            ".foo/*:deep(.bar)*/.baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo/*[*/.bar { color: red; }", "data-v-test"),
            ".foo/*[*/.bar[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo/*x*/:hover { color: red; }", "data-v-test"),
            ".foo/*x*/[data-v-test]:hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo /*x*/:hover { color: red; }", "data-v-test"),
            ".foo[data-v-test] :hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo/*,*/.bar, .baz) { color: red; }", "data-v-test"),
            ":is(.foo/*,*/.bar[data-v-test], .baz[data-v-test]) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo { &:is(.bar/*,*/.baz, .qux) { color: red; } }",
                "data-v-test",
            ),
            ".foo {\n&[data-v-test]:is(.bar/*,*/.baz, .qux) { color: red;\n}\n}"
        );
    }

    #[test]
    fn rewrites_nested_deep_container_pseudos_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo :deep(.bar)) { color: red; }", "data-v-test"),
            ":is(.foo[data-v-test] .bar) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":where(.foo :deep(.bar)) { color: red; }", "data-v-test",),
            ":where(.foo[data-v-test] .bar) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(:deep(.foo)) .bar { color: red; }", "data-v-test"),
            ":is([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":where(:deep(.foo)) .bar { color: red; }", "data-v-test",),
            ":where([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(:deep(.foo), .bar) .baz { color: red; }", "data-v-test",),
            ":is([data-v-test] .foo) .baz, :is(.bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":where(:deep(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":where([data-v-test] .foo) .baz, :where(.bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":not(:deep(.foo)) .bar { color: red; }", "data-v-test"),
            ":not([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":has(:deep(.foo)) .bar { color: red; }", "data-v-test"),
            ":has([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":has(:deep(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":has([data-v-test] .foo) .baz, :has(.bar) .baz[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_deep_container_special_branches_like_vue3() {
        let options = StyleCompileOptions {
            id: Some("data-v-test".into()),
            scoped: true,
            ..StyleCompileOptions::default()
        };

        let mixed = compile_style(
            ":is(:slotted(.foo), :deep(.bar), :global(.baz), .qux) { color: red; }",
            options.clone(),
        );
        assert_eq!(
            mixed.code,
            ":is(.foo[data-v-test-s],[data-v-test] .bar,.baz, .qux[data-v-test]) { color: red;\n}"
        );

        let scoped_anchor = compile_style(
            ".host:is(:deep(.foo), :global(.bar), :slotted(.baz), .qux) { color: red; }",
            options.clone(),
        );
        assert_eq!(
            scoped_anchor.code,
            ".host[data-v-test]:is( .foo,.bar,.baz, .qux) { color: red;\n}"
        );

        let nested_at_rule = compile_style(
            ":is(:deep(.foo), :slotted(.baz), .qux) { @media (min-width:1px){ .child { color:red; } } }",
            options.clone(),
        );
        assert_eq!(
            nested_at_rule.code,
            ":is([data-v-test] .foo,.baz[data-v-test-s], .qux[data-v-test]) {\n@media (min-width:1px){\n.child { color:red;\n}\n}\n}"
        );

        let direct_nested = compile_style(
            ":is(:deep(.foo), :global(.bar), :slotted(.baz), .qux) { color: blue; .child { color:red; } }",
            options,
        );
        assert_eq!(
            direct_nested.code,
            ":is([data-v-test] .foo,.bar,[data-v-test].baz[data-v-test-s], .qux[data-v-test])[data-v-test] { color: blue;\n.child { color:red;\n}\n}"
        );
    }

    #[test]
    fn rewrites_deep_container_split_pseudo_suffix_like_vue3() {
        let options = StyleCompileOptions {
            id: Some("data-v-test".into()),
            scoped: true,
            ..StyleCompileOptions::default()
        };

        let is_hover = compile_style(":is(:deep(.d), .n):hover { color:red; }", options.clone());
        assert_eq!(
            is_hover.code,
            ":is([data-v-test] .d):hover, :is(.n[data-v-test]):hover { color:red;\n}"
        );

        let where_before = compile_style(
            ":where(.x :deep(.d), :slotted(.s))::before { color:red; }",
            options.clone(),
        );
        assert_eq!(
            where_before.code,
            ":where(.x[data-v-test] .d)::before, :where(.s[data-v-test-s])::before { color:red;\n}"
        );

        let has_hover = compile_style(":has(:deep(.d), .n):hover { color:red; }", options.clone());
        assert_eq!(
            has_hover.code,
            ":has([data-v-test] .d):hover,[data-v-test]:has(.n):hover { color:red;\n}"
        );

        let has_normal_first = compile_style(
            ":has(.n,:deep(.d),.m):hover { color:red; }",
            options.clone(),
        );
        assert_eq!(
            has_normal_first.code,
            "[data-v-test]:has(.n):hover, :has([data-v-test] .d):hover,[data-v-test]:has(.m):hover { color:red;\n}"
        );

        let has_multiple_deep = compile_style(
            ":has(.n,:deep(.d),:deep(.e),.m):hover { color:red; }",
            options.clone(),
        );
        assert_eq!(
            has_multiple_deep.code,
            "[data-v-test]:has(.n):hover, :has([data-v-test] .d):hover, :has([data-v-test] .e):hover,[data-v-test]:has(.m):hover { color:red;\n}"
        );

        let direct_nested = compile_style(
            ":where(:deep(.d), :slotted(.s))::before { color: blue; .child { color: red; } }",
            options,
        );
        assert_eq!(
            direct_nested.code,
            ":where([data-v-test] .d)[data-v-test]::before, :where([data-v-test].s[data-v-test-s])::before { color: blue;\n.child { color: red;\n}\n}"
        );
    }

    #[test]
    fn rewrites_deep_passthrough_nested_at_rule_special_selectors_like_vue3() {
        let options = StyleCompileOptions {
            id: Some("data-v-test".into()),
            scoped: true,
            ..StyleCompileOptions::default()
        };

        let anchor = compile_style(
            ":deep(.d) { color: blue; & .child { color: red; } }",
            options.clone(),
        );
        assert_eq!(
            anchor.code,
            "[data-v-test] .d { color: blue;\n& .child { color: red;\n}\n}"
        );

        let commented_anchor = compile_style(
            ":deep(.d) { color: blue; /*x*/ &.active { color: red; } }",
            options.clone(),
        );
        assert_eq!(
            commented_anchor.code,
            "[data-v-test] .d { color: blue; /*x*/\n&.active { color: red;\n}\n}"
        );

        let container_anchor = compile_style(
            ".host :is(:deep(.d), .n):hover { color: blue; & .child { color: red; } }",
            options.clone(),
        );
        assert_eq!(
            container_anchor.code,
            ".host[data-v-test] :is([data-v-test] .d, .n[data-v-test]):hover { color: blue;\n& .child { color: red;\n}\n}"
        );

        let deep = compile_style(
            ":deep(.d) { @media (min-width:1px){ :deep(.inner) { color:red; } :global(.g) { color:blue; } :slotted(.s) { color:green; } } }",
            options.clone(),
        );
        assert_eq!(
            deep.code,
            "[data-v-test] .d {\n@media (min-width:1px){\n .inner { color:red;\n}\n.g { color:blue;\n}\n.s { color:green;\n}\n}\n}"
        );

        let prefixed = compile_style(
            ":deep(.d) { @media (min-width:1px){ .x :deep(.inner) { color:red; } .x:slotted(.s) { color:blue; } } }",
            options.clone(),
        );
        assert_eq!(
            prefixed.code,
            "[data-v-test] .d {\n@media (min-width:1px){\n.x .inner { color:red;\n}\n.x.s { color:blue;\n}\n}\n}"
        );

        let container = compile_style(
            ":is(:deep(.d), .n) { color: blue; @media (min-width:1px){ :deep(.inner) { color:red; } :global(.g) { color:green; } :slotted(.s) { color:yellow; } } }",
            options,
        );
        assert_eq!(
            container.code,
            ":is([data-v-test] .d, .n[data-v-test]) { color: blue;\n@media (min-width:1px){\n .inner { color:red;\n}\n.g { color:green;\n}\n.s { color:yellow;\n}\n}\n}"
        );
    }

    #[test]
    fn rewrites_deep_first_branch_and_nested_passthrough_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":deep(.foo, .bar) { color: red; }", "data-v-test"),
            "[data-v-test] .foo { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":deep(.foo) {\n  color: red;\n} ::v-deep .bar {\n  color: blue;\n}",
                "data-v-test",
            ),
            "[data-v-test] .foo {\n  color: red;\n} [data-v-test] .bar {\n  color: blue;\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(":deep(.foo, .bar) .baz { color: red; }", "data-v-test"),
            "[data-v-test] .foo .baz { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".host :deep(.foo) { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ".host[data-v-test] .foo { color: blue; .child { color: red; } }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":deep(.foo) { @media (min-width: 1px) { .bar { color: red; } } }",
                "data-v-test",
            ),
            "[data-v-test] .foo {\n@media (min-width: 1px) {\n.bar { color: red; }\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":deep(.foo) { @keyframes fade { to { opacity: 1; } } animation: fade 1s; }",
                "data-v-test",
            ),
            "[data-v-test] .foo {\n@keyframes fade-test {\nto { opacity: 1; }\n} animation: fade-test 1s;\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo >>> .bar { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ".foo .bar {\n&[data-v-test] { color: blue;\n}\n.child[data-v-test] { color: red;\n}\n}"
        );
    }

    #[test]
    fn emits_vue3_deprecated_deep_selector_warnings() {
        let result = compile_style(
            ">>> .foo { color: red; } ::v-deep .bar { color: blue; } :deep .baz { color: green; }",
            StyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                warn_deprecated_scoped_selectors: true,
                ..StyleCompileOptions::default()
            },
        );

        let messages = result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            vec![
                DEPRECATED_DEEP_COMBINATOR_MESSAGE,
                "::v-deep usage as a combinator has been deprecated. Use :deep(<inner-selector>) instead of ::v-deep <inner-selector>.",
                ":deep usage as a combinator has been deprecated. Use :deep(<inner-selector>) instead of :deep <inner-selector>.",
            ]
        );
        assert!(result.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR"
                && diagnostic.severity == vuec_diagnostics::Severity::Warning
        }));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn skips_deprecated_deep_warnings_outside_vue3_warning_mode() {
        let result = compile_style(
            ">>> .foo { color: red; } @keyframes fade { >>> { opacity: 1; } } :global(>>> .bar) { color: blue; }",
            StyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.diagnostics.is_empty());

        let warned = compile_style(
            "@keyframes fade { >>> { opacity: 1; } } :global(>>> .bar) { color: blue; }",
            StyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                warn_deprecated_scoped_selectors: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(warned.diagnostics.is_empty());
    }

    #[test]
    fn leaves_nested_slotted_pseudo_scoped_on_outer_selector() {
        let code =
            rewrite_scoped_selectors(":not(:slotted(.foo)) .bar { color: red; }", "data-v-test");

        assert_eq!(
            code,
            ":not(:slotted(.foo)) .bar[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_scoped_selectors_inside_container_at_rules() {
        let code =
            rewrite_scoped_selectors("@media print { .foo { color: #000; } }", "v-scope-xxx");

        assert!(code.contains(".foo[v-scope-xxx] { color: #000;"));
    }

    #[test]
    fn mounts_scope_on_correct_universal_selector_target() {
        assert_eq!(
            rewrite_scoped_selectors("* { color: red; }", "data-v-test"),
            "[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors("* .foo { color: red; }", "data-v-test"),
            ".foo[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors("*.foo { color: red; }", "data-v-test"),
            ".foo[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo * { color: red; }", "data-v-test"),
            ".foo[data-v-test] * { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo *.bar { color: red; }", "data-v-test"),
            ".foo *.bar[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_native_nested_scoped_rules_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(".foo { .bar { color: red; } }", "data-v-test"),
            ".foo {\n.bar[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo { color: blue; .bar { color: red; } color: green; }",
                "data-v-test",
            ),
            ".foo {\n&[data-v-test] { color: blue; color: green;\n}\n.bar[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo { &:hover { color: red; } }", "data-v-test"),
            ".foo {\n&[data-v-test]:hover { color: red;\n}\n}"
        );
    }

    #[test]
    fn rewrites_direct_nested_parent_selectors_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors("*.foo { color: blue; .bar { color: red; } }", "data-v-test"),
            ".foo {\n&[data-v-test] { color: blue;\n}\n.bar[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                "*.foo,* + .baz { color: blue; .bar { color: red; } }",
                "data-v-test",
            ),
            ".foo, + .baz {\n&[data-v-test] { color: blue;\n}\n.bar[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".host :slotted(.slot) { color: blue; .bar { color: red; } }",
                "data-v-test",
            ),
            ".host .slot {\n&[data-v-test] { color: blue;\n}\n.bar[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(:global(.g), :slotted(.s), * .item):hover { color: blue; .bar { color: red; } }",
                "data-v-test",
            ),
            ":is(.g,.s,.item):hover {\n&[data-v-test] { color: blue;\n}\n.bar[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo /*x*/ .bar { .child { color: red; } }", "data-v-test",),
            ".foo /*x*/ .bar {\n.child[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo /*x*/ .bar, *.baz { .child { color: red; } }",
                "data-v-test",
            ),
            ".foo  .bar,.baz {\n.child[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(.foo /*x*/ .bar, .baz) { .child { color: red; } }",
                "data-v-test",
            ),
            ":is(.foo /*x*/ .bar, .baz) {\n.child[data-v-test] { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(.foo /*x*/ .bar, *.baz) { .child { color: red; } }",
                "data-v-test",
            ),
            ":is(.foo  .bar,.baz) {\n.child[data-v-test] { color: red;\n}\n}"
        );
    }

    #[test]
    fn rewrites_direct_nested_first_normal_deep_containers_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(.foo, :deep(.bar), .baz) { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ":is(.foo,[data-v-test] .bar, .baz[data-v-test])[data-v-test] {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".host :where(:global(.g), :slotted(.s), :deep(.d), .tail) { color: blue; & .child { color: red; } }",
                "data-v-test",
            ),
            ".host[data-v-test] :where(.g,.s,[data-v-test] .d, .tail[data-v-test]) {\n& { color: blue;\n}\n& .child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":has(.foo, :deep(.bar)) { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ":has(.foo,[data-v-test] .bar)[data-v-test] {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":not(.foo, :deep(.bar)) { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ":not(.foo,[data-v-test] .bar)[data-v-test] {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(.foo, :deep(.bar), .baz):hover { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ":is(.foo):hover, :is([data-v-test] .bar)[data-v-test]:hover, :is(.baz[data-v-test]):hover {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(.foo, :deep(.bar), .baz).active { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ":is(.foo).active, :is([data-v-test] .bar)[data-v-test].active, :is(.baz).active[data-v-test] {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".host :where(.foo, :deep(.bar), :global(.g), :slotted(.s)):hover { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ".host[data-v-test] :where(.foo,[data-v-test] .bar,.g,[data-v-test].s[data-v-test-s]):hover {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":has(.foo, :deep(.bar), .baz):hover { color: blue; .child { color: red; } }",
                "data-v-test",
            ),
            ":has(.foo):hover, :has([data-v-test] .bar)[data-v-test]:hover,[data-v-test]:has(.baz):hover {\n& { color: blue;\n}\n.child { color: red;\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(:global(.g),.foo,:deep(.d)).active { color:blue; .child{color:red;} }",
                "data-v-test",
            ),
            ":is(:global(.g)).active, :is(.foo).active, :is([data-v-test] .d)[data-v-test].active {\n& { color:blue;\n}\n.child{color:red;}\n}"
        );
    }

    #[test]
    fn rewrites_scoped_nested_at_rules_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo { color: blue; @media (min-width: 1px) { .bar { color: red; } } }",
                "data-v-test",
            ),
            ".foo[data-v-test] { color: blue;\n@media (min-width: 1px) {\n.bar[data-v-test] { color: red;\n}\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo { @media (min-width: 1px) { &:hover { color: red; } } }",
                "data-v-test",
            ),
            ".foo[data-v-test] {\n@media (min-width: 1px) {\n&[data-v-test]:hover { color: red;\n}\n}\n}"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".foo { @keyframes fade { to { opacity: 1; } } animation: fade 1s; }",
                "data-v-test",
            ),
            ".foo[data-v-test] {\n@keyframes fade-test {\nto { opacity: 1;\n}\n} animation: fade-test 1s;\n}"
        );
    }

    #[test]
    fn scoped_style_resource_limits_have_exact_boundaries() {
        let source = ":is(.a){}";
        let scope_id = "data-v-x";
        let exact = ScopedStyleLimits {
            max_source_bytes: source.len(),
            max_scope_id_bytes: scope_id.len(),
            max_syntax_depth: 1,
            max_recursive_scan_bytes: 15,
            max_keyframes: 0,
            max_keyframe_bytes: 0,
        };
        assert_eq!(
            rewrite_scoped_selectors_with_limits(source, scope_id, exact).unwrap(),
            ":is(.a[data-v-x]){}"
        );

        for limits in [
            ScopedStyleLimits {
                max_source_bytes: source.len() - 1,
                ..exact
            },
            ScopedStyleLimits {
                max_scope_id_bytes: scope_id.len() - 1,
                ..exact
            },
            ScopedStyleLimits {
                max_syntax_depth: 0,
                ..exact
            },
            ScopedStyleLimits {
                max_recursive_scan_bytes: 14,
                ..exact
            },
        ] {
            let error = rewrite_scoped_selectors_with_limits(source, scope_id, limits)
                .expect_err("one-short resource limit must fail");
            assert_eq!(error.code, "VUEC_STYLE_SCOPED_LIMIT");
        }
    }

    #[test]
    fn scoped_style_depth_combines_block_and_selector_frames() {
        let source = "@media x { :is(:where(.a)) {} }";
        assert!(validate_scoped_style_resources(
            source,
            "data-v-x",
            ScopedStyleLimits {
                max_syntax_depth: 3,
                ..ScopedStyleLimits::default()
            },
        )
        .is_ok());
        let error = validate_scoped_style_resources(
            source,
            "data-v-x",
            ScopedStyleLimits {
                max_syntax_depth: 2,
                ..ScopedStyleLimits::default()
            },
        )
        .expect_err("combined block and selector depth must be bounded");
        assert_eq!(error.code, "VUEC_STYLE_SCOPED_LIMIT");

        assert!(validate_scoped_style_resources(
            r#".a { content: "{{(("; /* {(( */ }"#,
            "data-v-x",
            ScopedStyleLimits {
                max_syntax_depth: 1,
                ..ScopedStyleLimits::default()
            },
        )
        .is_ok());
    }

    #[test]
    fn scoped_style_rejects_adversarial_nesting_before_recursive_passes() {
        let depth = STYLE_SCOPED_MAX_SYNTAX_DEPTH + 1;
        let mut source = "@media x {".repeat(depth);
        source.push_str(".a {}");
        source.push_str(&"}".repeat(depth));

        assert!(rewrite_scoped_selectors(&source, "data-v-x").is_empty());
        let result = compile_style(
            &source,
            StyleCompileOptions {
                id: Some("data-v-x".into()),
                scoped: true,
                modules: true,
                vars: vec!["color".into()],
                source_map: true,
                source_map_base_offset: 17,
                warn_deprecated_scoped_selectors: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(result.code.is_empty());
        assert!(result.map.is_none());
        assert!(result.modules.is_none());
        assert!(result.vars.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_SCOPED_LIMIT");
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(FileId(0), 17, 18))
        );
    }

    #[test]
    fn scoped_keyframes_use_indexed_definition_and_reference_lookups() {
        let mut source = String::new();
        for index in 0..4_096 {
            source.push_str(&format!("@keyframes k{index} {{}}"));
        }
        source.push_str(".a { animation: k0 1s; animation-name: k4095; }");

        let mut budget = ScopedStyleBudget::new(ScopedStyleLimits::default());
        let keyframes = collect_scoped_keyframes(&source, "test", &mut budget).unwrap();
        assert_eq!(keyframes.len(), 4_096);
        assert_eq!(lookup_keyframe_name("k0", &keyframes).unwrap(), "k0-test");
        assert_eq!(
            lookup_keyframe_name("k4095", &keyframes).unwrap(),
            "k4095-test"
        );

        let output = rewrite_scoped_selectors(&source, "data-v-test");
        assert!(output.contains("@keyframes k0-test {}"));
        assert!(output.contains("@keyframes k4095-test {}"));
        assert!(output.contains("animation: k0-test 1s;"));
        assert!(output.contains("animation-name: k4095-test;"));
    }

    #[test]
    fn scoped_keyframe_suffix_detection_preserves_existing_names() {
        assert!(scoped_keyframe_name_has_suffix("fade-test", "test"));
        assert!(scoped_keyframe_name_has_suffix("fade-", ""));
        assert!(!scoped_keyframe_name_has_suffix("fadecontest", "test"));

        let mut budget = ScopedStyleBudget::new(ScopedStyleLimits::default());
        let keyframes = collect_scoped_keyframes(
            "@keyframes fade-test {} @keyframes fade {} @-webkit-keyframes fade {}",
            "test",
            &mut budget,
        )
        .unwrap();
        assert_eq!(keyframes.len(), 1);
        assert_eq!(keyframes.get("fade").unwrap(), "fade-test");
    }

    #[test]
    fn scoped_keyframe_state_limits_have_exact_boundaries() {
        let source = "@keyframes a {} @keyframes bb {} @keyframes a {}";
        let exact_limits = ScopedStyleLimits {
            max_keyframes: 2,
            max_keyframe_bytes: 10,
            ..ScopedStyleLimits::default()
        };
        let mut exact_budget = ScopedStyleBudget::new(exact_limits);
        let keyframes = collect_scoped_keyframes(source, "x", &mut exact_budget).unwrap();
        assert_eq!(keyframes.len(), 2);
        assert_eq!(
            (exact_budget.keyframes, exact_budget.keyframe_bytes),
            (2, 10)
        );

        for limits in [
            ScopedStyleLimits {
                max_keyframes: 1,
                ..exact_limits
            },
            ScopedStyleLimits {
                max_keyframe_bytes: 9,
                ..exact_limits
            },
        ] {
            let mut budget = ScopedStyleBudget::new(limits);
            let error = collect_scoped_keyframes(source, "x", &mut budget)
                .expect_err("one-short keyframe budget must fail");
            assert_eq!(error.code, "VUEC_STYLE_SCOPED_LIMIT");
            assert_eq!((budget.keyframes, budget.keyframe_bytes), (1, 4));
        }
    }

    #[test]
    fn scoped_keyframe_limit_failure_is_atomic() {
        let source = "@keyframes a {} @keyframes bb {} .a >>> .b {}";
        let limits = ScopedStyleLimits {
            max_keyframes: 2,
            max_keyframe_bytes: 9,
            ..ScopedStyleLimits::default()
        };
        let result = compile_style_with_scoped_limits(
            source,
            StyleCompileOptions {
                id: Some("data-v-x".into()),
                scoped: true,
                modules: true,
                vars: vec!["color".into()],
                source_map: true,
                source_map_base_offset: 3,
                warn_deprecated_scoped_selectors: true,
                ..StyleCompileOptions::default()
            },
            limits,
        );
        assert!(result.code.is_empty());
        assert!(result.map.is_none());
        assert!(result.modules.is_none());
        assert!(result.vars.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_SCOPED_LIMIT");
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(FileId(0), 3, 4))
        );

        let unscoped = compile_style_with_scoped_limits(
            ".a {}",
            StyleCompileOptions::default(),
            ScopedStyleLimits {
                max_source_bytes: 0,
                max_scope_id_bytes: 0,
                max_syntax_depth: 0,
                max_recursive_scan_bytes: 0,
                max_keyframes: 0,
                max_keyframe_bytes: 0,
            },
        );
        assert!(unscoped.errors.is_empty());
        assert!(unscoped.diagnostics.is_empty());
        assert_eq!(unscoped.code, ".a {}");
    }
