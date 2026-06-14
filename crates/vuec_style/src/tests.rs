#[cfg(test)]
mod tests {
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
    fn compiles_vars_modules_and_map() {
        let result = compile_style(
            ".a { color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-x".into()),
                scoped: true,
                modules: true,
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(result.code.contains("[data-v-x]"));
        assert!(result.code.contains("var(--x-color)"));
        let modules = result.modules.expect("css modules map");
        assert!(modules.get("a").is_some_and(|value| value.contains("_a_")));
        assert_eq!(result.vars, vec!["color"]);
        assert!(result.map.is_some());
    }

    #[test]
    fn compiles_css_modules_default_local_and_global_pseudo() {
        let result = compile_style(
            ".red { color: red }\n.green { color: green }\n:global(.blue) { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(modules
            .get("green")
            .is_some_and(|value| value.contains("_green_")));
        assert!(!modules.contains_key("blue"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compiles_css_modules_global_scope_with_local_and_camel_case_only() {
        let result = compile_style(
            ":local(.foo-bar) { color: red }\n.baz-qux { color: green }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    generate_scoped_name: Some("[name]__[local]__[hash:base64:5]".into()),
                    locals_convention: "camelCaseOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("__foo-bar__")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(!modules.contains_key("bazQux"));
        assert!(result.code.contains(".baz-qux { color: green }"));
    }

    #[test]
    fn compiles_css_modules_leaves_class_attribute_selectors_global() {
        let result = compile_style(
            "[class=\"btn\"] { color: red }\n:local([class='forced']) { color: blue }\n[class~=tag] { color: green }\n.btn { color: black }",
            StyleCompileOptions {
                filename: Some("src/Attr.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("btn")
            .is_some_and(|value| value.contains("_btn_")));
        assert!(!modules.contains_key("forced"));
        assert!(!modules.contains_key("tag"));
        assert!(result.code.contains("[class=\"btn\"] { color: red }"));
        assert!(result.code.contains("[class='forced'] { color: blue }"));
        assert!(result.code.contains("[class~=tag] { color: green }"));
        assert!(result.code.contains("._btn_"));
        assert!(result.code.contains("{ color: black }"));
    }

    #[test]
    fn compiles_css_modules_global_module_paths_for_matching_file() {
        let result = compile_style(
            ".button { color: red }\n:local(.forced) { color: blue }",
            StyleCompileOptions {
                filename: Some("src/theme.global.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(!modules.contains_key("button"));
        assert!(modules
            .get("forced")
            .is_some_and(|value| value.contains("_forced_")));
        assert!(result.code.contains(".button { color: red }"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compiles_css_modules_global_module_paths_uses_entry_scope_for_imported_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("theme.global.css");
        std::fs::write(
            &dep,
            ".dep { color: blue; }\n:local(.forced) { color: green; }",
        )
        .expect("write dep");

        let result = compile_style(
            ".button { composes: forced from \"./theme.global.css\"; color: red; }",
            StyleCompileOptions {
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(!modules.contains_key("dep"));
        assert!(button.contains("_button_"));
        assert!(button.contains("_forced_"));
        assert!(result.code.contains("._dep_"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compiles_css_modules_id_selectors_like_official() {
        let result = compile_style(
            "#panel { color: red }\n.button#item { color: blue }",
            StyleCompileOptions {
                filename: Some("src/Selectors.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_aau0c_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_aau0c_2")
        );
        assert_eq!(
            modules.get("item").map(String::as_str),
            Some("_item_aau0c_1")
        );
        assert!(result.code.contains("#_panel_aau0c_1"));
        assert!(result.code.contains("._button_aau0c_2#_item_aau0c_1"));
    }

    #[test]
    fn compiles_css_modules_global_scope_local_id_only() {
        let result = compile_style(
            ":local(#panel) #plain { color: red }",
            StyleCompileOptions {
                filename: Some("src/Selectors.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_qt8vi_1")
        );
        assert!(!modules.contains_key("plain"));
        assert!(result.code.contains("#_panel_qt8vi_1 #plain"));
    }

    #[test]
    fn compiles_css_modules_export_global_ids() {
        let result = compile_style(
            ":global(#panel) { color: red }",
            StyleCompileOptions {
                filename: Some("src/Selectors.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("panel").map(String::as_str), Some("panel"));
        assert!(result.code.contains("#panel"));
    }

    #[test]
    fn compiles_css_modules_generate_scoped_name_hash_prefix_like_official() {
        let result = compile_style(
            ".button { color: red }",
            StyleCompileOptions {
                id: Some("data-v-probe".into()),
                filename: Some("src/Comp.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    generate_scoped_name: Some("[local]__[hash:base64:5]".into()),
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("button__2G66Z")
        );
        assert!(result.code.contains(".button__2G66Z"));
    }

    #[test]
    fn ignores_css_modules_hash_prefix_for_default_scoped_names_like_official() {
        let base = compile_style(
            ".button { color: red }",
            StyleCompileOptions {
                id: Some("data-v-probe".into()),
                filename: Some("src/Comp.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let prefixed = compile_style(
            ".button { color: red }",
            StyleCompileOptions {
                id: Some("data-v-probe".into()),
                filename: Some("src/Comp.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(base.code, prefixed.code);
        assert_eq!(base.modules, prefixed.modules);
    }

    #[test]
    fn compiles_css_modules_keyframes_and_animation_names_like_official() {
        let result = compile_style(
            "@keyframes fade { from { opacity: 0 } to { opacity: 1 } }\n.button { animation-name: fade; }",
            StyleCompileOptions {
                filename: Some("src/Anim.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

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
    fn compiles_css_modules_animation_shorthand_keywords_like_official() {
        let result = compile_style(
            ".button { animation: infinite infinite, ease ease, none 1s, fade 1s; }",
            StyleCompileOptions {
                filename: Some("src/Anim.vue".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_11sc8_1")
        );
        assert_eq!(
            modules.get("infinite").map(String::as_str),
            Some("_infinite_11sc8_1")
        );
        assert_eq!(
            modules.get("ease").map(String::as_str),
            Some("_ease_11sc8_1")
        );
        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_11sc8_1")
        );
        assert!(!modules.contains_key("none"));
        assert!(result.code.contains(
            "animation: infinite _infinite_11sc8_1, ease _ease_11sc8_1, none 1s, _fade_11sc8_1 1s"
        ));
    }

    #[test]
    fn compiles_css_modules_global_scope_local_keyframes_only() {
        let result = compile_style(
            "@keyframes :local(fade) { from { opacity: 0 } to { opacity: 1 } }\n.button { animation-name: fade; }",
            StyleCompileOptions {
                filename: Some("src/Anim.vue".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_3cm9u_1")
        );
        assert!(!modules.contains_key("button"));
        assert!(result.code.contains("@keyframes _fade_3cm9u_1"));
        assert!(result.code.contains(".button { animation-name: fade"));
    }

    #[test]
    fn compiles_css_modules_dashes_locals_convention() {
        let result = compile_style(
            ".foo-bar { color: red }\n.foo_bar { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashes".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let foo_bar_scoped = modules.get("foo-bar").expect("original dashed export");

        assert_eq!(modules.get("fooBar"), Some(foo_bar_scoped));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
        assert_ne!(modules.get("fooBar"), modules.get("foo_bar"));
        assert!(result.code.contains("._foo-bar_"));
        assert!(result.code.contains("._foo_bar_"));
    }

    #[test]
    fn compiles_css_modules_dashes_only_locals_convention() {
        let result = compile_style(
            ".foo-bar { color: red }\n.foo_bar { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("_foo-bar_")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
        assert_ne!(modules.get("fooBar"), modules.get("foo_bar"));
        assert!(result.code.contains("._foo-bar_"));
        assert!(result.code.contains("._foo_bar_"));
    }

    #[test]
    fn compiles_css_modules_locals_convention_alias_collisions_like_official() {
        for locals_convention in ["camelCase", "dashes"] {
            let result = compile_style(
                ".foo-bar { color: red }\n.fooBar { color: blue }",
                StyleCompileOptions {
                    id: Some("test".into()),
                    filename: Some("test.css".into()),
                    modules: true,
                    modules_options: CssModulesOptions {
                        locals_convention: locals_convention.into(),
                        ..CssModulesOptions::default()
                    },
                    ..StyleCompileOptions::default()
                },
            );
            let modules = result.modules.expect("css modules map");

            assert!(modules
                .get("foo-bar")
                .is_some_and(|value| value.contains("_foo-bar_")));
            assert!(modules
                .get("fooBar")
                .is_some_and(|value| value.contains("_fooBar_")));
        }
    }

    #[test]
    fn compiles_css_modules_export_globals() {
        let result = compile_style(
            ".local :global(.global) { color: red }\n:global(.blue) { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("local")
            .is_some_and(|value| value.contains("_local_")));
        assert_eq!(modules.get("global").map(String::as_str), Some("global"));
        assert_eq!(modules.get("blue").map(String::as_str), Some("blue"));
        assert!(result.code.contains("._local_"));
        assert!(result.code.contains(".global"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compiles_css_modules_export_globals_with_global_scope_and_convention() {
        let result = compile_style(
            ".foo-bar .foo_bar { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    locals_convention: "dashesOnly".into(),
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("fooBar").map(String::as_str), Some("foo-bar"));
        assert_eq!(modules.get("foo_bar").map(String::as_str), Some("foo_bar"));
        assert!(!modules.contains_key("foo-bar"));
        assert_eq!(result.code, ".foo-bar .foo_bar { color: blue }");
    }

    #[test]
    fn compiles_css_modules_local_composes() {
        let result = compile_style(
            ".base { color: blue }\n.button { composes: base; color: red }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(base.contains("_base_"));
        assert!(button.contains("_button_"));
        assert!(button.contains(base));
        assert!(!result.code.contains("composes"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn compiles_css_modules_global_and_chained_composes() {
        let result = compile_style(
            ".base { composes: global(reset); }\n.button { composes: base global(extra); }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(base.contains("_base_"));
        assert!(base.contains("reset"));
        assert!(button.contains("_button_"));
        assert!(button.contains(base));
        assert!(button.contains("extra"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn reports_css_modules_composes_on_complex_selector() {
        let result = compile_style(
            ".button.extra { composes: base; }\n.next { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec![
                "composition is only allowed when selector is single :local class name not in \":local(.button):local(.extra)\""
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn reports_css_modules_missing_composes_class() {
        let result = compile_style(
            ".button { composes: missing; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                source_map_file_id: Some(FileId(11)),
                source_map_base_offset: 20,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec!["referenced class name \"missing\" in composes not found"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        let start = ".button { composes: ".len();
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(
                FileId(11),
                20 + start,
                20 + start + "missing".len()
            ))
        );
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn reports_css_modules_late_composes_class() {
        let result = compile_style(
            ".button { composes: next; }\n.next { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec!["referenced class name \"next\" in composes not found"]
        );
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn compiles_css_modules_missing_external_composes_class_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }").expect("write dep");

        let result = compile_style(
            ".button { composes: missing from \"./dep.css\"; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty());
        assert!(result.diagnostics.is_empty());
        assert!(button.contains("undefined"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compiles_css_modules_icss_exports() {
        let result = compile_style(
            ":export { primary: red; spacing: 1px; }\n.button { color: primary; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(modules.get("spacing").map(String::as_str), Some("1px"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":export"));
        assert!(result.code.contains("color: primary"));
    }

    #[test]
    fn compiles_css_modules_local_values_like_official() {
        let result = compile_style(
            r#"@value primary: red; @value accent: primary; @value query: (min-width: 1px);
@media query { .button::before { content: "accent"; /* accent */ color: accent; } }
.accent { border-color: accent; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("value.module.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(modules.get("accent").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("content: \"red\""));
        assert!(result.code.contains("/* accent */ color: red"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn compiles_css_modules_values_as_single_pass_replacements() {
        let result = compile_style(
            "@value accent: primary; @value primary: red; .button { color: accent; background: primary; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("value-order.module.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("accent").map(String::as_str), Some("primary"));
        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(result.code.contains("color: primary"));
        assert!(result.code.contains("background: red"));
    }

    #[test]
    fn compiles_css_modules_value_imports_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("tokens.css");
        std::fs::write(
            &dep,
            "@value primary: red; @value query: (min-width: 1px); .remote { color: primary; }",
        )
        .expect("write dep");
        let entry = dir.path().join("entry.css");
        let result = compile_style(
            r#"@value primary, query, remote as external, missing from "./tokens.css";
@value accent: primary;
@media query { .button { composes: external; color: accent; outline-color: missing; } }
.external { border-color: primary; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(entry.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let external = modules.get("external").expect("external export");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(modules.get("accent").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert_eq!(
            modules.get("missing").map(String::as_str),
            Some("undefined")
        );
        assert!(external.contains("_remote_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_") && value.contains(external)));
        assert!(!result.code.contains("@value"));
        assert!(!result.code.contains("_external_"));
        assert!(!result.code.contains("; }"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: red"));
        assert!(result.code.contains("outline-color: i__const_missing_3"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn compiles_css_modules_missing_value_import_composes_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dep = dir.path().join("tokens.css");
        std::fs::write(&dep, "@value primary: red; .remote { color: primary; }")
            .expect("write dep");
        let entry = dir.path().join("entry.css");
        let result = compile_style(
            r#"@value missing from "./tokens.css";
.button { composes: missing; color: missing; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(entry.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
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
    fn compiles_css_modules_icss_exports_with_locals_convention() {
        let result = compile_style(
            ":export { theme-color: red; }\n.button { color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert_eq!(modules.get("themeColor").map(String::as_str), Some("red"));
        assert!(!modules.contains_key("theme-color"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_relative_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");

        let result = compile_style(
            ".button { composes: dep from \"./dep.css\"; color: token; }\n.plain { color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(modules
            .get("plain")
            .is_some_and(|value| value.contains("_plain_")));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
        assert!(!result.code.contains("composes"));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
        assert!(!result.code.contains("composes"));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_package_main() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","main":"theme.css"}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
        assert!(!result.code.contains(":import"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_package_exports_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":"./theme.css"}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_conditional_exports_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"default":"./theme.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_exports_require_condition_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("require.css"),
            ".dep { color: blue; }\n:export { token: requireGreen; }",
        )
        .expect("write require dep");
        std::fs::write(
            package_dir.join("default.css"),
            ".dep { color: red; }\n:export { token: defaultRed; }",
        )
        .expect("write default dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"require":"./require.css","default":"./default.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.contains("color: requireGreen"));
        assert!(!result.code.contains("defaultRed"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_exports_condition_order_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("node.css"),
            ".dep { color: blue; }\n:export { token: nodePurple; }",
        )
        .expect("write node dep");
        std::fs::write(
            package_dir.join("require.css"),
            ".dep { color: red; }\n:export { token: requireGreen; }",
        )
        .expect("write require dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"node":"./node.css","require":"./require.css","default":"./require.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains("color: nodePurple"));
        assert!(!result.code.contains("requireGreen"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_node_modules_nested_exports_conditions_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            package_dir.join("require.css"),
            ".dep { color: blue; }\n:export { token: nestedRequire; }",
        )
        .expect("write require dep");
        std::fs::write(
            package_dir.join("default.css"),
            ".dep { color: red; }\n:export { token: fallbackDefault; }",
        )
        .expect("write default dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{".":{"node":{"require":"./require.css","default":"./default.css"},"default":"./default.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ":import(\"vuec-css-fixture\") { imported: dep; shade: token; }\n.button { composes: imported; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains("color: nestedRequire"));
        assert!(!result.code.contains("fallbackDefault"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_package_exports_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":"./dist/theme.css"}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_conditional_exports_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":{"default":"./dist/theme.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_exports_require_condition_subpath()
    {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("require.css"),
            ".dep { color: requireGreen; }\n:export { token: requireGreen; }",
        )
        .expect("write require dep");
        std::fs::write(
            dist_dir.join("default.css"),
            ".dep { color: defaultRed; }\n:export { token: defaultRed; }",
        )
        .expect("write default dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":{"require":"./dist/require.css","default":"./dist/default.css"}}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("requireGreen"));
        assert!(!result.code.contains("defaultRed"));
    }

    #[test]
    fn compiles_css_modules_external_composes_from_node_modules_wildcard_exports_subpath() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(
            dist_dir.join("theme.css"),
            ".dep { color: blue; }\n:export { token: green; }",
        )
        .expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./*.css":"./dist/*.css"}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._button_"));
    }

    #[test]
    fn css_modules_node_modules_exports_blocks_unexported_subpath_fallback() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&package_dir).expect("package dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(package_dir.join("theme.css"), ".dep { color: blue; }").expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./other.css":"./theme.css"}}"#,
        )
        .expect("write package");

        let result = compile_style(
            ".button { composes: dep from \"vuec-css-fixture/theme.css\"; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(!button.contains("_dep_"));
        assert!(result.code.contains("composes"));
    }

    #[test]
    fn compiles_css_modules_multiple_external_composes_from_relative_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n.extra { color: green; }").expect("write dep");

        let result = compile_style(
            ".button { composes: dep extra from \"./dep.css\"; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(button.contains("_extra_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("._extra_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compiles_css_modules_composes_from_global() {
        let result = compile_style(
            ".button { composes: reset utility from global; color: red; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("reset"));
        assert!(button.contains("utility"));
        assert!(!result.code.contains("composes"));
        assert!(result.code.contains("color: red"));
    }

    #[test]
    fn compiles_css_modules_icss_imports_from_relative_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");

        let result = compile_style(
            ":import(\"./dep.css\") { imported: dep; shade: token; }\n.button { color: shade; }\n.other { composes: imported; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let other = modules.get("other").expect("other export");

        assert!(other.contains("_other_"));
        assert!(other.contains("_dep_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains("color: green"));
        assert!(!result.code.contains(":import"));
    }

    #[test]
    fn compiles_css_modules_icss_import_symbols_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(
            &dep,
            ".dep { color: blue; }\n:export { token: green; query: (min-width: 1px); }",
        )
        .expect("write dep");

        let result = compile_style(
            r#":import("./dep.css") { imported: dep; shade: token; mq: query; }
.shade { color: red; }
.imported { border-color: shade; }
@media mq { .button::before { content: "shade"; color: shade; } }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains(".green { color: red"));
        assert!(result.code.contains("._dep_"));
        assert!(result.code.contains("border-color: green"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("content: \"green\""));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compiles_css_modules_missing_icss_import_symbols_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");

        let result = compile_style(
            r#":import("./dep.css") { imported: dep; shade: nope; color: token; mq: missing; }
.shade { color: red; }
.imported { border-color: color; }
.button { composes: shade; color: shade; }
@media mq { .panel { color: shade; } }
:export { out: shade; importedOut: color; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty());
        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(button.contains("_button_"));
        assert!(button.contains("undefined"));
        assert_eq!(modules.get("out").map(String::as_str), Some("undefined"));
        assert_eq!(
            modules.get("importedOut").map(String::as_str),
            Some("green")
        );
        assert!(modules
            .get("panel")
            .is_some_and(|value| value.contains("_panel_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains(":export"));
        assert!(!result.code.contains("composes"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(result.code.contains(".shade { color: red"));
        assert!(result.code.contains("border-color: green"));
        assert!(result.code.contains("color: shade"));
        assert!(result.code.contains("@media mq"));
    }

    #[test]
    fn compiles_css_modules_native_nested_rules_like_official() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");

        let result = compile_style(
            r#".foo { color: blue; .bar { color: red; } &.active { color: green; } @media (min-width: 1px) { :global(.global) { color: black; } :local(.inner) { color: white; } } color: yellow; }"#,
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

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
    fn reports_css_modules_native_nested_composes_like_official() {
        let result = compile_style(
            ".foo { .bar { composes: foo; color: red; } }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                source_map_file_id: Some(FileId(7)),
                source_map_base_offset: 10,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec![
                "composition is not allowed in nested rule \n\n:local(.bar) { composes: foo; color: red;\n}"
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        let start = ".foo { .bar {".len();
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(
                FileId(7),
                10 + start,
                10 + start + " composes: foo".len()
            ))
        );
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn compiles_css_modules_relative_imports_before_locals_convention_projection() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(
            &dep,
            ".foo-bar { color: blue; }\n:export { theme-color: green; }",
        )
        .expect("write dep");

        let result = compile_style(
            ":import(\"./dep.css\") { shade: theme-color; }\n.button { composes: foo-bar from \"./dep.css\"; color: shade; }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some(filename.to_string_lossy().to_string()),
                modules: true,
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_foo-bar_"));
        assert!(result.code.contains("color: green"));
        assert!(!modules.contains_key("foo-bar"));
        assert!(!modules.contains_key("fooBar"));
    }

    #[test]
    fn source_map_tracks_original_style_source_lines() {
        let source = ".a { color: red; }\n.b { color: blue; }";
        let result = compile_style(
            source,
            StyleCompileOptions {
                filename: Some("component.vue".into()),
                source_map_source: Some(format!("<style>\n{source}\n</style>")),
                source_map_file_id: Some(FileId(7)),
                source_map_base_offset: "<style>\n".len(),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        let map = result.map.expect("style source map");

        assert_eq!(map.sources, vec!["component.vue"]);
        assert_eq!(
            map.sources_content
                .as_ref()
                .and_then(|sources| sources[0].as_ref()),
            Some(&format!("<style>\n{source}\n</style>"))
        );
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("first mapping");
        assert_eq!(first.source, "component.vue");
        assert_eq!(first.line, 1);
        assert_eq!(first.column, 0);
        let second = map
            .original_position(vuec_source::GeneratedPosition::new(1, 0))
            .unwrap()
            .expect("second mapping");
        assert_eq!(second.line, 2);
        assert_eq!(second.column, 0);
    }

    #[test]
    fn preserves_plain_css_imports_without_missing_import_diagnostics() {
        let source = ".a { color: red; }\n@import \"./not-missing.css\";\n@import \"missing.css\";";
        let result = compile_style(source, StyleCompileOptions::default());

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("@import \"./not-missing.css\";"));
        assert!(result.code.contains("@import \"missing.css\";"));
    }

    #[test]
    fn less_unresolved_import_reports_resolver_source_span() {
        let source = ".a { color: red; }\n  @import \"./theme\";\n.b { color: blue; }";
        let result = compile_style(
            source,
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                source_map_file_id: Some(FileId(9)),
                source_map_base_offset: 100,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(
            result.errors,
            vec!["Less import could not be resolved: ./theme"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "VUEC_STYLE_IMPORT_RESOLVE");
        let start = ".a { color: red; }\n  ".len();
        let end = start + "@import \"./theme\";".len();
        assert_eq!(
            diagnostic.span,
            Some(Span::new(FileId(9), 100 + start, 100 + end))
        );
    }

    #[test]
    fn preprocesses_vue27_style_languages_before_css_transforms() {
        let less = compile_style(
            "@red: rgb(255, 0, 0);\n.color { color: @red; }",
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(less.errors.is_empty());
        assert!(less.code.contains("color: #ff0000;"));
        assert!(less.map.is_some());

        let scss = compile_style(
            "$red: red;\n.color { color: $red; .child { width: 1px; } }",
            StyleCompileOptions {
                preprocess_lang: Some("scss".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(scss.code.contains("color: red;"));
        assert!(scss.code.contains(".color .child"));

        let sass = compile_style(
            "$red: red\n.color\n  color: $red",
            StyleCompileOptions {
                preprocess_lang: Some("sass".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(sass.code.contains("color: red;"));

        let stylus = compile_style(
            "red-color = rgb(255, 0, 0);\n.color\n  color: red-color",
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(stylus.code.contains("color: #f00;"));
    }

    #[test]
    fn preprocesses_less_variables_nested_selectors_and_media() {
        let result = compile_style(
            r#"
@red: rgb(255, 0, 0);
.card, .panel {
  @gap: 8px;
  color: @red;
  padding: @gap;
  &:hover {
    color: blue;
  }
  .title {
    margin: @gap;
  }
  @media (min-width: 600px) {
    display: block;
    .title {
      color: @red;
    }
  }
}
.other {
  color: @red;
}
"#,
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".card, .panel {"));
        assert!(result.code.contains("color: #ff0000;"));
        assert!(result.code.contains("padding: 8px;"));
        assert!(result.code.contains(".card:hover, .panel:hover {"));
        assert!(result.code.contains(".card .title, .panel .title {"));
        assert!(result.code.contains("@media (min-width: 600px) {"));
        assert!(result.code.contains("display: block;"));
        assert!(result.code.contains(".other {"));
        assert!(!result.code.contains("@red"));
        assert!(!result.code.contains("@gap"));
    }

    #[test]
    fn preprocesses_less_additional_data_imports_and_load_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&shared_dir).expect("shared dir");
        let base = src_dir.join("component.less");
        let local_import = src_dir.join("local.less");
        let load_path_import = shared_dir.join("tokens.less");
        std::fs::write(
            &local_import,
            r#"
.imported {
  border-color: @brand;
}
"#,
        )
        .expect("write local import");
        std::fs::write(
            &load_path_import,
            r#"
@space: 12px;
.shared {
  margin: @space;
}
"#,
        )
        .expect("write load path import");

        let result = compile_style(
            r#"
@import "./local.less";
@import "tokens";
@import "https://example.com/reset.css";
.root {
  color: @brand;
  padding: @space;
}
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("less".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("@brand: red;".into()),
                    load_paths: vec![shared_dir.to_string_lossy().into_owned()],
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result
            .code
            .contains("@import \"https://example.com/reset.css\";"));
        assert!(result.code.contains(".imported {"));
        assert!(result.code.contains("border-color: red;"));
        assert!(result.code.contains(".shared {"));
        assert!(result.code.contains("margin: 12px;"));
        assert!(result.code.contains("padding: 12px;"));
        let mut expected = vec![
            std::fs::canonicalize(local_import)
                .expect("canonical local import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
            std::fs::canonicalize(load_path_import)
                .expect("canonical load path import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
        ];
        expected.sort();
        assert_eq!(result.dependencies, expected);
    }

    #[test]
    fn preprocesses_stylus_variables_nested_selectors_and_media() {
        let result = compile_style(
            r#"
red-color = rgb(255, 0, 0)
gap = 8px
.card, .panel
  color red-color
  padding: gap
  &:hover
    color blue
  .title
    margin gap
  @media (min-width: 600px)
    display block
    .title
      color red-color
.other
  color red-color
"#,
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".card, .panel {"));
        assert!(result.code.contains("color: #f00;"));
        assert!(result.code.contains("padding: 8px;"));
        assert!(result.code.contains(".card:hover, .panel:hover {"));
        assert!(result.code.contains(".card .title, .panel .title {"));
        assert!(result.code.contains("@media (min-width: 600px) {"));
        assert!(result.code.contains("display: block;"));
        assert!(result.code.contains(".other {"));
        assert!(!result.code.contains("red-color"));
        assert!(!result.code.contains("gap"));
    }

    #[test]
    fn preprocesses_stylus_additional_data_imports_and_load_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&shared_dir).expect("shared dir");
        let base = src_dir.join("component.styl");
        let local_import = src_dir.join("local.styl");
        let load_path_import = shared_dir.join("tokens.styl");
        std::fs::write(
            &local_import,
            r#"
.imported
  border-color brand
"#,
        )
        .expect("write local import");
        std::fs::write(
            &load_path_import,
            r#"
space = 12px
.shared
  margin space
"#,
        )
        .expect("write load path import");

        let result = compile_style(
            r#"
@import "./local"
@import "tokens"
@import "https://example.com/reset.css"
.root
  color brand
  padding space
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("stylus".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("brand = red".into()),
                    load_paths: vec![shared_dir.to_string_lossy().into_owned()],
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result
            .code
            .contains("@import \"https://example.com/reset.css\";"));
        assert!(result.code.contains(".imported {"));
        assert!(result.code.contains("border-color: red;"));
        assert!(result.code.contains(".shared {"));
        assert!(result.code.contains("margin: 12px;"));
        assert!(result.code.contains("padding: 12px;"));
        let mut expected = vec![
            std::fs::canonicalize(local_import)
                .expect("canonical local import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
            std::fs::canonicalize(load_path_import)
                .expect("canonical load path import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
        ];
        expected.sort();
        assert_eq!(result.dependencies, expected);
    }

    #[test]
    fn preprocesses_scss_additional_data_and_import_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("test.scss");
        let import = dir.path().join("import.scss");
        std::fs::write(&import, ".imported { color: $red; }\n").expect("write import");

        let result = compile_style(
            r#"
@import "./import.scss";
.square {
  @include square(100px);
}
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("scss".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some(
                        r#"
$red: red;
@mixin square($size) {
  width: $size;
  height: $size;
}
"#
                        .into(),
                    ),
                    ..StylePreprocessOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("color: red;"));
        assert!(result.code.contains("width: 100px;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .to_string();
        assert_eq!(
            result.dependencies,
            vec![normalize_native_dependency_path(Path::new(
                &resolved_import
            ))]
        );
    }

    #[test]
    fn collects_css_vars_like_vue27() {
        let vars = collect_css_vars(
            r#"
            /* color: v-bind(ignored); */
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind((a + b) / 2 + 'px');
              height: v-bind("count.toString(");
              border: v-bind(color);
            }
            "#,
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn collects_css_vars_like_vue3_with_line_comments() {
        let vars = collect_css_vars_with_options(
            r#"
            // color: v-bind(ignored);
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind    ((a + b) / 2 + 'px' );
              height: v-bind("count.toString(");
            }
            "#,
            CssVarCollectOptions {
                ignore_line_comments: true,
            },
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn collects_css_vars_across_interstitial_block_comments() {
        let vars = collect_css_vars_with_options(
            concat!(
                ".foo { color: v-bind/**/(color); ",
                "font-size: v-bind /*x*/ ('font.size'); ",
                "width: v-bind/**/ (size); }"
            ),
            CssVarCollectOptions {
                ignore_line_comments: true,
            },
        );

        assert_eq!(vars, vec!["color", "font.size", "size"]);
    }

    #[test]
    fn rewrites_css_vars_with_vue27_names() {
        let code = rewrite_css_vars(
            ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
            "test",
            false,
        );
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
        assert_eq!(gen_css_var_name("xxxxxxxx", "color", true), "4003f1a6");
        assert_eq!(gen_css_var_name("xxxxxxxx", "font.size", true), "41b6490a");
    }

    #[test]
    fn rewrites_css_vars_across_comment_separated_call_gaps() {
        let code = rewrite_css_vars_with_options(
            concat!(
                ".foo { color: v-bind /*x*/ (color); ",
                "font-size: v-bind /**/ /**/ ('font.size'); ",
                "width: v-bind/**/ (size); ",
                "height: v-bind/**/(height); }"
            ),
            "test",
            CssVarRewriteOptions {
                is_prod: false,
                name_style: CssVarNameStyle::Vue3Escaped,
                ignore_line_comments: true,
            },
        );

        assert!(code.contains("var(--test-color)"));
        assert!(code.contains(r"var(--test-font\.size)"));
        assert!(code.contains("var(--test-size)"));
        assert!(code.contains("v-bind/**/(height)"));
    }

    #[test]
    fn rewrites_css_vars_with_vue3_escaped_names() {
        let code = rewrite_css_vars_with_options(
            concat!(
                ".foo { color: v-bind(color); font-size: v-bind('font.size'); ",
                "font-weight: v-bind(_φ); width: calc(v-bind(foo + 'px') - 3px); }\n",
                "// color: v-bind(ignored)\n",
                ".bar { width: v-bind(width); }"
            ),
            "test",
            CssVarRewriteOptions {
                is_prod: false,
                name_style: CssVarNameStyle::Vue3Escaped,
                ignore_line_comments: true,
            },
        );

        assert!(code.contains("var(--test-color)"));
        assert!(code.contains(r"var(--test-font\.size)"));
        assert!(code.contains("var(--test-_φ)"));
        assert!(code.contains(r"var(--test-foo\ \+\ \'px\')"));
        assert!(code.contains("// color: v-bind(ignored)"));
        assert!(code.contains("var(--test-width)"));
        assert_eq!(
            gen_css_var_name_with_style("xxxxxxxx", "color", true, CssVarNameStyle::Vue3Escaped),
            "v4003f1a6"
        );
    }
}
