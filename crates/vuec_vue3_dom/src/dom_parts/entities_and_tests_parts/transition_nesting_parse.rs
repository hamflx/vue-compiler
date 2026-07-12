    #[test]
    fn transform_transition_projection_filters_comments_and_whitespace() {
        let projection = transition_projection(vec![
            json!({ "type": 3, "content": "ignored" }),
            json!({ "type": 2, "content": "\n  " }),
            transition_element_child(vec![]),
        ]);

        assert_eq!(projection["keepChildren"], json!([2]));
        assert_eq!(projection["errors"], json!([]));
        assert_eq!(projection["injectPersisted"], json!(false));
    }

    #[test]
    fn transition_transform_detaches_filtered_arena_children() {
        let mut ast = parse(
            template_source(
                "transition.vue",
                "<transition><!-- ignored --><div /></transition>",
            ),
            &DomCompilerOptions::default(),
        );
        let transition_id = ast.root_node().unwrap().children[0];
        let comment_id = ast
            .node(transition_id)
            .unwrap()
            .children
            .iter()
            .copied()
            .find(|child_id| {
                ast.node(*child_id)
                    .is_some_and(|child| matches!(child.kind, Vue3AstKind::Comment(_)))
            })
            .expect("transition comment child");

        transform_transition_children(&mut ast, &mut TransformContext::default());

        assert_eq!(ast.node(comment_id).unwrap().parent, None);
        assert_eq!(ast.node(comment_id).unwrap().index_in_parent, 0);
        assert_eq!(ast.validate_tree(), Ok(()));
    }

    #[test]
    fn transform_transition_projection_reports_invalid_children() {
        let projection = transition_projection(vec![
            transition_element_child(vec![]),
            json!({
                "type": 1,
                "tag": "span",
                "tagType": 0,
                "props": [],
                "children": [],
                "loc": {
                    "start": { "line": 1, "column": 25, "offset": 24 },
                    "end": { "line": 1, "column": 38, "offset": 37 },
                    "source": "<span></span>"
                }
            }),
        ]);

        assert_eq!(projection["errors"][0]["code"], json!(63));
        assert_eq!(projection["errors"][0]["loc"]["start"]["offset"], json!(12));
        assert_eq!(projection["errors"][0]["loc"]["end"]["offset"], json!(37));

        let for_child = transition_projection(vec![json!({
            "type": 11,
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 40, "offset": 39 },
                "source": "<div v-for=\"i in items\"/>"
            }
        })]);
        assert_eq!(for_child["errors"][0]["code"], json!(63));
    }

    #[test]
    fn transform_transition_projection_handles_if_branch_shape() {
        let valid_if = transition_projection(vec![json!({
            "type": 9,
            "branches": [
                { "children": [transition_element_child(vec![])] },
                { "children": [transition_element_child(vec![])] }
            ],
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 80, "offset": 79 },
                "source": ""
            }
        })]);
        assert_eq!(valid_if["errors"], json!([]));

        let invalid_template_if = transition_projection(vec![json!({
            "type": 9,
            "branches": [
                { "children": [] }
            ],
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 40, "offset": 39 },
                "source": ""
            }
        })]);
        assert_eq!(invalid_template_if["errors"][0]["code"], json!(63));
    }

    #[test]
    fn transform_transition_projection_injects_persisted_for_v_show_child() {
        let projection = transition_projection(vec![transition_element_child(vec![json!({
            "type": 7,
            "name": "show",
        })])]);

        assert_eq!(projection["errors"], json!([]));
        assert_eq!(projection["injectPersisted"], json!(true));
    }

    #[test]
    fn is_valid_html_nesting_projection_matches_vue_dom_table() {
        for (parent, child, valid) in [
            ("form", "form", false),
            ("form", "input", true),
            ("p", "div", false),
            ("p", "span", true),
            ("a", "a", false),
            ("button", "button", false),
            ("table", "tr", false),
            ("table", "tbody", true),
            ("td", "td", false),
            ("tr", "td", true),
            ("tbody", "td", false),
            ("tbody", "tr", true),
            ("li", "li", false),
            ("li", "ul", true),
            ("h1", "h6", false),
            ("h1", "div", true),
            ("svg", "div", false),
            ("svg", "g", true),
            ("foreignObject", "div", true),
            ("g", "p", true),
            ("span", "dt", false),
            ("dl", "dt", true),
            ("template", "tr", true),
        ] {
            assert_eq!(
                is_valid_html_nesting_projection(&json!({
                    "parent": parent,
                    "child": child,
                }))["valid"],
                json!(valid),
                "{parent} > {child}"
            );
        }
    }

    #[test]
    fn validate_html_nesting_projection_reports_invalid_children() {
        let projection = validate_html_nesting_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 0,
                "loc": { "source": "<div></div>" }
            },
            "parent": {
                "type": 1,
                "tag": "p",
                "tagType": 0
            }
        }));

        assert_eq!(
            projection["warnings"][0]["loc"]["source"],
            json!("<div></div>")
        );
        assert!(projection["warnings"][0]["message"]
            .as_str()
            .unwrap()
            .contains("<div> cannot be child of <p>"));

        let valid = validate_html_nesting_projection(&json!({
            "node": { "type": 1, "tag": "hr", "tagType": 0 },
            "parent": { "type": 1, "tag": "select", "tagType": 0 }
        }));
        assert_eq!(valid["warnings"], json!([]));

        let component_child = validate_html_nesting_projection(&json!({
            "node": { "type": 1, "tag": "Child", "tagType": 1 },
            "parent": { "type": 1, "tag": "p", "tagType": 0 }
        }));
        assert_eq!(component_child["warnings"], json!([]));
    }

    #[test]
    fn parse_uses_dom_parser_defaults() {
        let ast = parse(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<input><hello/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            &DomCompilerOptions::default(),
        );
        let root = ast.node(ast.root).expect("root");
        let input = ast.node(root.children[0]).expect("input");
        let hello = ast.node(root.children[1]).expect("hello");

        assert!(input.children.is_empty());
        assert!(matches!(
            &hello.kind,
            Vue3AstKind::Element(element)
                if element.tag == "hello" && element.tag_type == Vue3ElementType::Component
        ));
    }
