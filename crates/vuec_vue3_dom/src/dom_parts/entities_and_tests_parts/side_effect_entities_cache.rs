    #[test]
    fn ignore_side_effect_tags_projection_removes_native_script_and_style() {
        for tag in ["script", "style"] {
            let projection = ignore_side_effect_tags_projection(&json!({
                "node": {
                    "type": 1,
                    "tag": tag,
                    "tagType": 0,
                    "loc": { "source": format!("<{tag}></{tag}>") }
                }
            }));

            assert_eq!(projection["remove"], json!(true));
            assert_eq!(projection["errors"][0]["code"], json!(64));
            assert_eq!(
                projection["errors"][0]["loc"]["source"],
                json!(format!("<{tag}></{tag}>"))
            );
        }
    }

    #[test]
    fn ignore_side_effect_tags_projection_keeps_non_native_side_effect_names() {
        for node in [
            json!({ "type": 1, "tag": "div", "tagType": 0 }),
            json!({ "type": 1, "tag": "script", "tagType": 1 }),
            json!({ "type": 1, "tag": "style", "tagType": 3 }),
            json!({ "type": 2, "content": "script" }),
        ] {
            let projection = ignore_side_effect_tags_projection(&json!({ "node": node }));

            assert_eq!(projection["remove"], json!(false));
            assert_eq!(projection["errors"], json!([]));
        }
    }

    #[test]
    fn side_effect_transform_detaches_removed_arena_children() {
        let mut ast = parse(
            template_source(
                "side-effect.vue",
                "<div><script>run()</script><span /></div>",
            ),
            &DomCompilerOptions::default(),
        );
        let script_id = ast
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    &node.kind,
                    Vue3AstKind::Element(element) if element.tag == "script"
                )
            })
            .map(|node| node.id)
            .expect("script node");

        remove_side_effect_nodes(&mut ast, &mut TransformContext::default());

        assert_eq!(ast.node(script_id).unwrap().parent, None);
        assert_eq!(ast.node(script_id).unwrap().index_in_parent, 0);
        assert_eq!(ast.validate_tree(), Ok(()));
    }

    #[test]
    fn decode_html_browser_projection_decodes_text_and_attribute_entities() {
        for (raw, decoded) in [
            (" abc  123 ", " abc  123 "),
            ("&", "&"),
            ("&amp;", "&"),
            ("&amp;amp;", "&amp;"),
            ("&lt;", "<"),
            ("&amp;lt;", "&lt;"),
            ("&gt;", ">"),
            ("&nbsp;", "\u{00a0}"),
            ("&quot;", "\""),
            ("&apos;", "'"),
            ("&Eacute;", "\u{00c9}"),
            ("&#xc9;", "\u{00c9}"),
            ("&#201;", "\u{00c9}"),
        ] {
            let projection = decode_html_browser_projection(&json!({ "raw": raw }));

            assert_eq!(projection["decoded"], json!(decoded), "{raw}");
        }

        let attr = decode_html_browser_projection(&json!({
            "raw": "<strong>&lt;strong&gt;&amp;&lt;/strong&gt;</strong>",
            "asAttr": true,
        }));
        assert_eq!(
            attr["decoded"],
            json!("<strong><strong>&</strong></strong>")
        );
    }

    #[test]
    fn dom_compiler_ast_cache_hits_for_same_parse_input() {
        let mut compiler = DomCompiler::new();
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.mode = "module".into();
        let source = template_source("cached.vue", "<div>{{ msg }}</div>");

        let first = compiler.compile(source.clone(), options.clone());
        let second = compiler.compile(source, options);

        assert_eq!(first.code, second.code);
        assert_eq!(
            compiler.cache_stats(),
            DomAstCacheStats {
                ast_hits: 1,
                ast_misses: 1,
                ast_invalidations: 0,
            }
        );
        assert_eq!(compiler.ast_cache_len(), 1);
    }

    #[test]
    fn dom_compiler_ast_cache_invalidates_changed_same_file_source() {
        let mut compiler = DomCompiler::new();
        let options = DomCompilerOptions::default();
        let first = template_source("cached.vue", "<div>{{ one }}</div>");
        let second = template_source("cached.vue", "<section>{{ two }}</section>");

        let first_result = compiler.compile(first, options.clone());
        let second_result = compiler.compile(second, options);

        assert_ne!(first_result.code, second_result.code);
        assert!(second_result.code.contains("section"));
        assert_eq!(
            compiler.cache_stats(),
            DomAstCacheStats {
                ast_hits: 0,
                ast_misses: 2,
                ast_invalidations: 1,
            }
        );
        assert_eq!(compiler.ast_cache_len(), 1);
    }

    #[test]
    fn dom_compiler_ast_cache_key_separates_parse_options() {
        let mut compiler = DomCompiler::new();
        let source = template_source("cached.vue", "<div><!--x-->{{ msg }}</div>");
        let with_comments = DomCompilerOptions::default();
        let mut without_comments = DomCompilerOptions::default();
        without_comments.core.comments = false;

        let with_comments_result = compiler.compile(source.clone(), with_comments);
        let without_comments_result = compiler.compile(source, without_comments);

        assert_ne!(
            with_comments_result.ast_summary,
            without_comments_result.ast_summary
        );
        assert_eq!(
            compiler.cache_stats(),
            DomAstCacheStats {
                ast_hits: 0,
                ast_misses: 2,
                ast_invalidations: 0,
            }
        );
        assert_eq!(compiler.ast_cache_len(), 2);
    }

    #[test]
    fn extracts_dom_directives() {
        let attrs = vec![
            TemplateAttribute {
                name: "@click.stop".into(),
                value: Some("save".into()),
            },
            TemplateAttribute {
                name: "v-model".into(),
                value: Some("checked".into()),
            },
        ];
        let directives = extract_directives(&attrs);
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[0].name, "on");
        assert_eq!(directives[0].modifiers, vec!["stop"]);
    }

    #[test]
    fn compile_records_dom_summary() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./a.png"><input v-model="value">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions::default(),
        );
        assert!(result.ast_summary.starts_with("dom:"));
        assert!(result.ast_summary.contains("v-model:vModelText"));
        assert!(!result.code.contains("data-vuec-dom"));
    }
