    #[test]
    fn compile_rewrites_explicit_base_assets_in_module_mode() {
        let mut options = DomCompilerOptions {
            asset_url_options: AssetUrlOptions {
                base: Some("/foo".into()),
                ..AssetUrlOptions::default()
            },
            ..DomCompilerOptions::default()
        };
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./bar.png"><img src="bar.png"><img src="~bar.png"><img src="@theme/bar.png"><img src="/bar.png"><img src="data:image/png;base64,i">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains(r#"src: "/foo/bar.png""#));
        assert!(result.code.contains(r#"src: "bar.png""#));
        assert!(result.code.contains("import _imports_0 from 'bar.png'"));
        assert!(result
            .code
            .contains("import _imports_1 from '@theme/bar.png'"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("src: _imports_1"));
        assert!(result.code.contains(r#"src: "/bar.png""#));
        assert!(result.code.contains(r#"src: "data:image/png;base64,i""#));
        assert!(!result.code.contains(r#"src: "~bar.png""#));
        assert!(!result.code.contains(r#"src: "@theme/bar.png""#));
    }

    #[test]
    fn compile_transforms_asset_urls_to_imports_in_module_mode() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r##"<img src="./bar.png"><img src="~fixtures/logo.png"><img src="@theme/bar.png"><img src="./icons.svg#heart"><use href="#local"></use>"##.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result
            .code
            .contains("import _imports_1 from 'fixtures/logo.png'"));
        assert!(result
            .code
            .contains("import _imports_2 from '@theme/bar.png'"));
        assert!(result.code.contains("import _imports_3 from './icons.svg'"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("src: _imports_1"));
        assert!(result.code.contains("src: _imports_2"));
        assert!(result.code.contains(r#"src: _imports_3 + '#heart'"#));
        assert!(result.code.contains(r##"href: "#local""##));
        assert!(!result.code.contains("_ctx._imports_"));
    }

    #[test]
    fn compile_caches_static_children_with_asset_url_imports() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<div><img src="./bar.png"><span title="static">ok</span></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("-1"));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("8 /* PROPS */"));
        assert!(!result.code.contains("[\"src\"]"));
    }

    #[test]
    fn compile_stringifies_static_children_with_asset_url_imports() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><img src="./bar.png" srcset="./bar.png, ./icons.svg#heart 2x" />{}</div>"#,
                    r#"<span title="static">ok</span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(
            result.code.contains("_createStaticVNode"),
            "{}",
            result.code
        );
        assert!(
            result
                .code
                .contains("const _hoisted_1 = _imports_0 + ', ' + _imports_1 + '#heart' + ' 2x'"),
            "{}",
            result.code
        );
        assert!(
            result.code.contains(
                r##"_createStaticVNode("<img src=\"" + _imports_0 + "\" srcset=\"" + _hoisted_1 + "\"><span title=\"static\">ok</span>"##
            ),
            "{}",
            result.code
        );
        assert!(!result.code.contains("src: _imports_0"));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("_ctx._imports_1"));
    }

    #[test]
    fn compile_stringifies_multiple_static_chunks_around_dynamic_child() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    "<div>{}{{{{ msg }}}}{}</div>",
                    r#"<span class="foo"></span>"#.repeat(5),
                    r#"<span class="bar"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert_eq!(result.code.matches("_createStaticVNode(").count(), 2);
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span>\", 5)"));
    }

    #[test]
    fn compile_bails_stringify_static_invalid_p_child_placement() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                        + "<span><div class=\"block\"></div></span>"
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("_createElementVNode(\"p\""));
        assert!(result
            .code
            .contains("_createElementVNode(\"div\", { class: \"block\" })"));
    }

    #[test]
    fn compile_stringifies_static_children_when_transform_hoist_requested() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn compile_stringifies_static_constant_bindings_when_transform_hoist_requested() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><div :style="`color:red;`">{}</div></div>"#,
                    r#"<span :class="[{ foo: true }, { bar: true }]">{{ 1 }} + {{ false }}</span>"#
                        .repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<div style=\"color:red;\"><span class=\"foo bar\">1 + false</span>"#));
    }

    #[test]
    fn compile_stringifies_static_children_with_scope_id() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.mode = "module".into();
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        options.core.scope_id = Some("data-v-test".into());
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><div :style="`color:red;`">{}</div></div>"#,
                    r#"<span class="foo">ok</span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(
            r#"<div style=\"color:red;\" data-v-test><span class=\"foo\" data-v-test>ok</span>"#
        ));
    }

    #[test]
    fn compile_stringifies_static_svg_namespace_children_by_default() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><svg width="50" height="50" viewBox="0 0 50 50" fill="none" xmlns="http://www.w3.org/2000/svg">{}</svg></div>"#,
                    r##"<rect width="50" height="50" fill="#C4C4C4"></rect>"##.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(r#"<svg width=\"50\" height=\"50\" viewBox=\"0 0 50 50\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"#));
        assert!(result
            .code
            .contains(r##"<rect width=\"50\" height=\"50\" fill=\"#C4C4C4\"></rect>"##));
    }

    #[test]
    fn compile_transforms_srcset_imports_in_module_mode() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png, ./icons.svg#heart 2x, /absolute.png 3x, data:image/png;base64,i 4x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(result.code.matches("import _imports_0").count() == 1);
        assert!(result.code.contains(
            r#"srcset: _imports_0 + ', ' + _imports_1 + '#heart' + ' 2x, ' + "/absolute.png" + ' 3x, ' + "data:image/png;base64,i" + ' 4x'"#
        ));
    }

    #[test]
    fn compile_rewrites_asset_url_base_with_hosts_and_hashes() {
        let cases = [
            (
                "http://localhost:3000/src/",
                "./logo.png",
                r#"src: "http://localhost:3000/src/logo.png""#,
            ),
            (
                "http://localhost:3000",
                "./logo.png",
                r#"src: "http://localhost:3000/logo.png""#,
            ),
            (
                "http://localhost",
                "./logo.png",
                r#"src: "http://localhost/logo.png""#,
            ),
            (
                "//localhost",
                "./logo.png",
                r#"src: "//localhost/logo.png""#,
            ),
            (
                "/foo",
                "./icons.svg#heart",
                r#"src: "/foo/icons.svg#heart""#,
            ),
        ];

        for (index, (base, url, expected)) in cases.iter().enumerate() {
            let result = compile(
                TemplateSource {
                    filename: format!("asset-base-{index}.vue"),
                    source: format!(r#"<img src="{url}">"#),
                    file_id: FileId(index as u32),
                    base_offset: 0,
                },
                DomCompilerOptions {
                    asset_url_options: AssetUrlOptions {
                        base: Some((*base).into()),
                        ..AssetUrlOptions::default()
                    },
                    ..DomCompilerOptions::default()
                },
            );

            assert!(
                result.code.contains(expected),
                "base {base} url {url} generated:\n{}",
                result.code
            );
        }
    }

    #[test]
    fn compile_rewrites_srcset_base_when_all_processable_urls_are_dot_relative() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img srcset="./logo.png, ./logo.png 2x, /logo.png 3x, data:image/png;base64,i 4x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            r#"srcset: "/foo/logo.png, /foo/logo.png 2x, /logo.png 3x, data:image/png;base64,i 4x""#
        ));
    }

    #[test]
    fn compile_rewrites_srcset_base_independently_of_asset_tag_options() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    tags: BTreeMap::new(),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "./logo.png""#));
        assert!(result.code.contains(r#"srcset: "/foo/logo.png 2x""#));
    }

    #[test]
    fn compile_rewrites_mixed_srcset_base_candidates_and_imports_alias_candidates() {
        let mut options = DomCompilerOptions {
            asset_url_options: AssetUrlOptions {
                base: Some("/foo".into()),
                ..AssetUrlOptions::default()
            },
            ..DomCompilerOptions::default()
        };
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img srcset="@/logo.png 1x, ./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from '@/logo.png'"));
        assert!(result
            .code
            .contains(r#"srcset: _imports_0 + ' 1x, ' + "/foo/logo.png" + ' 2x'"#));
    }

    #[test]
    fn compile_transforms_asset_url_options_for_custom_tags() {
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);
        let mut options = DomCompilerOptions {
            asset_url_options: AssetUrlOptions {
                tags,
                ..AssetUrlOptions::default()
            },
            ..DomCompilerOptions::default()
        };
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<foo bar="~baz"></foo>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from 'baz'"));
        assert!(result.code.contains("bar: _imports_0"));
    }

    #[test]
    fn compile_respects_disabled_asset_url_transform() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./bar.png">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                transform_asset_urls: false,
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "./bar.png""#));
        assert!(!result.code.contains("/foo/bar.png"));
    }

    #[test]
    fn parse_marks_dom_transition_builtins() {
        let ast = parse(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<transition/><transition-group/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            &DomCompilerOptions::default(),
        );
        let tags = ast
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3AstKind::Element(element) => Some((
                    element.tag.as_str(),
                    element.tag_type == vuec_ast::Vue3ElementType::Component,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(tags, vec![("transition", true), ("transition-group", true)]);
    }

    #[test]
    fn compile_reports_transition_invalid_children_diagnostics() {
        let cases = [
            ("<transition><div>hey</div><div>hey</div></transition>", true),
            ("<transition><div v-for=\"i in items\">hey</div></transition>", true),
            (
                "<transition><div v-if=\"a\" v-for=\"i in items\">hey</div><div v-else v-for=\"i in items\">hey</div></transition>",
                true,
            ),
            ("<transition><template v-if=\"ok\"></template></transition>", true),
            (
                "<transition><template v-if=\"a\"></template><template v-else></template></transition>",
                true,
            ),
            (
                "<transition><div v-if=\"one\">hey</div><div v-if=\"other\">hey</div></transition>",
                true,
            ),
            ("<transition><div>hey</div></transition>", false),
            ("<transition><div v-if=\"a\">hey</div></transition>", false),
            (
                "<transition><div v-if=\"a\">hey</div><div v-else-if=\"b\">hey</div><div v-else>hey</div></transition>",
                false,
            ),
            (
                "<transition><div v-if=\"a\">hey</div><div v-else>hey</div></transition>",
                false,
            ),
            ("<transition>\u{00a0}<div>foo</div></transition>", true),
            (
                "<transition><!-- foo --> <!-- bar --><div>foo bar</div></transition>",
                false,
            ),
        ];
        for (index, (source, should_warn)) in cases.iter().enumerate() {
            let result = compile(
                TemplateSource {
                    filename: format!("case-{index}.vue"),
                    source: (*source).into(),
                    file_id: FileId(index as u32),
                    base_offset: 0,
                },
                DomCompilerOptions::default(),
            );

            let has_warning = result.diagnostics.iter().any(|diagnostic| {
                diagnostic.message == "<Transition> expects exactly one child element or component."
            });
            assert_eq!(has_warning, *should_warn, "case {index}: {source}");
        }
    }

    #[test]
    fn compile_reports_invalid_native_v_model_diagnostics() {
        let result = compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div v-model="baz"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions::default(),
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "58");
        assert_eq!(
            result.diagnostics[0].message,
            "v-model can only be used on <input>, <textarea> and <select> elements."
        );
        assert_eq!(
            result.diagnostics[0].span,
            Some(vuec_source::Span::new(FileId(0), 5, 18))
        );
    }

    #[test]
    fn compile_allows_v_model_on_configured_custom_elements() {
        let mut options = DomCompilerOptions::default();
        options.core.custom_elements = vec!["my-input".into()];
        let result = compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<my-input v-model="baz"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.code.contains("vModelText"));
        assert!(result.code.contains("_withDirectives"));
    }

    #[test]
    fn compile_suppresses_invalid_native_v_model_after_binding_errors() {
        let mut options = DomCompilerOptions::default();
        options
            .core
            .binding_metadata
            .insert("foo".into(), "literal-const".into());
        let result = compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div v-model="foo"/><div v-model="foo + bar"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            ["45", "42"]
        );
    }
