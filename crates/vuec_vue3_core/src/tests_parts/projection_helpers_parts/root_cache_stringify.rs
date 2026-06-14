    #[test]
    fn root_codegen_projection_marks_single_visible_root_fragment() {
        let root = json!({
            "children": [
                { "type": 3 },
                { "type": 1, "tagType": 0 },
                { "type": 3 }
            ]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "fragment", "patchFlag": 2112 })
        );
    }

    #[test]
    fn get_constant_type_projection_handles_static_interpolation_and_props() {
        let interpolation = get_constant_type_projection(&json!({
            "node": {
                "type": 5,
                "content": { "type": 4, "content": "1", "constType": 3 }
            },
            "context": {}
        }));
        assert_eq!(interpolation["constantType"], json!(3));

        let static_props = get_constant_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 0,
                "props": [],
                "children": [],
                "codegenNode": {
                    "type": 13,
                    "isBlock": false,
                    "props": {
                        "type": 15,
                        "properties": [{
                            "type": 16,
                            "key": { "type": 4, "content": "id", "isStatic": true },
                            "value": { "type": 4, "content": "foo", "isStatic": true }
                        }]
                    }
                }
            },
            "context": {}
        }));
        assert_eq!(static_props["constantType"], json!(3));
    }

    #[test]
    fn cache_static_projection_caches_static_child_arrays() {
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [
                        {
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        },
                        {
                            "type": 1,
                            "tag": "i",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }
                    ],
                    "codegenNode": {
                        "type": 13,
                        "isBlock": true,
                        "children": [{ "type": 1 }, { "type": 1 }]
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"],
            json!([
                {
                    "kind": "setPatchFlag",
                    "path": ["children", "0", "children", "0", "codegenNode"],
                    "patchFlag": -1
                },
                {
                    "kind": "setPatchFlag",
                    "path": ["children", "0", "children", "1", "codegenNode"],
                    "patchFlag": -1
                },
                {
                    "kind": "cacheChildrenArray",
                    "path": ["children", "0", "codegenNode", "children"],
                    "childrenPath": ["children", "0", "children"],
                    "needArraySpread": true
                }
            ])
        );
    }

    #[test]
    fn cache_static_projection_hoists_props_and_dynamic_props() {
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [
                        {
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": {
                                "type": 13,
                                "patchFlag": 512,
                                "props": {
                                    "type": 15,
                                    "properties": [{
                                        "type": 16,
                                        "key": { "type": 4, "content": "id", "isStatic": true },
                                        "value": { "type": 4, "content": "foo", "isStatic": true }
                                    }]
                                }
                            }
                        },
                        {
                            "type": 1,
                            "tag": "p",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": {
                                "type": 13,
                                "patchFlag": 8,
                                "dynamicProps": "[\"foo\"]"
                            }
                        }
                    ],
                    "codegenNode": { "type": 13, "isBlock": true }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"],
            json!([
                {
                    "kind": "hoistProps",
                    "path": ["children", "0", "children", "0", "codegenNode", "props"]
                },
                {
                    "kind": "hoistDynamicProps",
                    "path": ["children", "0", "children", "1", "codegenNode", "dynamicProps"]
                }
            ])
        );
    }

    #[test]
    fn cache_static_projection_caches_dynamic_template_slot_returns() {
        let dynamic_slot = json!({
            "type": 8,
            "children": ["foo + ", { "type": 4, "content": "bar", "constType": 0 }]
        });
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "Comp",
                    "tagType": 1,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "template",
                        "tagType": 3,
                        "props": [{
                            "type": 7,
                            "name": "slot",
                            "arg": dynamic_slot
                        }],
                        "children": [{
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }]
                    }],
                    "codegenNode": {
                        "type": 13,
                        "children": {
                            "type": 15,
                            "properties": [{
                                "key": dynamic_slot,
                                "value": {
                                    "type": 18,
                                    "returns": [{ "type": 1 }]
                                }
                            }]
                        }
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(projection["operations"][0]["kind"], json!("setPatchFlag"));
        assert_eq!(
            projection["operations"][1],
            json!({
                "kind": "cacheSlotReturns",
                "ownerPath": ["children", "0"],
                "slot": {
                    "kind": "dynamic",
                    "node": dynamic_slot
                },
                "needArraySpread": true
            })
        );
    }

    #[test]
    fn cache_static_projection_downgrades_static_svg_blocks_except_with_directives() {
        let static_svg = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "svg",
                        "tagType": 0,
                        "props": [],
                        "children": [],
                        "codegenNode": { "type": 13, "isBlock": true }
                    }],
                    "codegenNode": {
                        "type": 13,
                        "isBlock": true,
                        "children": [{ "type": 1 }]
                    }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            static_svg["operations"][0],
            json!({
                "kind": "setBlock",
                "path": ["children", "0", "children", "0", "codegenNode"],
                "isBlock": false
            })
        );

        let svg_with_directive = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "svg",
                        "tagType": 0,
                        "props": [{ "type": 7, "name": "foo" }],
                        "children": [{
                            "type": 1,
                            "tag": "path",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }],
                        "codegenNode": {
                            "type": 13,
                            "isBlock": true,
                            "children": [{ "type": 1 }]
                        }
                    }],
                    "codegenNode": { "type": 13, "isBlock": true }
                }]
            },
            "context": {}
        }));
        let svg_codegen_path = json!(["children", "0", "children", "0", "codegenNode"]);
        assert!(svg_with_directive["operations"]
            .as_array()
            .expect("operations")
            .iter()
            .all(|operation| operation["path"] != svg_codegen_path));
        assert_eq!(
            svg_with_directive["operations"][1],
            json!({
                "kind": "cacheChildrenArray",
                "path": ["children", "0", "children", "0", "codegenNode", "children"],
                "childrenPath": ["children", "0", "children", "0", "children"],
                "needArraySpread": true
            })
        );
    }

    #[test]
    fn stringify_static_projection_stringifies_cached_adjacent_children() {
        let children = (0..STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
            .map(|_| {
                json!({
                    "type": 1,
                    "tag": "span",
                    "tagType": 0,
                    "ns": 0,
                    "props": [{
                        "type": 6,
                        "name": "class",
                        "value": { "content": "foo" }
                    }],
                    "children": [],
                    "codegenNode": { "type": 20, "index": 0 }
                })
            })
            .collect::<Vec<_>>();
        let projection = stringify_static_projection(&json!({
            "children": children,
            "context": {}
        }));
        let expected_html =
            r#"<span class="foo"></span>"#.repeat(STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT);

        assert_eq!(
            projection["operations"],
            json!([{
                "kind": "stringifyCachedChildRange",
                "start": 0,
                "count": STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT,
                "html": quote_string(&expected_html),
                "domNodes": STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT
            }])
        );
    }

    #[test]
    fn stringify_static_projection_stringifies_parent_cached_child_tree() {
        let children = vec![json!({
            "type": 1,
            "tag": "div",
            "tagType": 0,
            "ns": 0,
            "props": [],
            "children": (0..STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
                .map(|_| json!({
                    "type": 1,
                    "tag": "span",
                    "tagType": 0,
                    "ns": 0,
                    "props": [{
                        "type": 6,
                        "name": "class",
                        "value": { "content": "foo" }
                    }],
                    "children": []
                }))
                .collect::<Vec<_>>()
        })];
        let projection = stringify_static_projection(&json!({
            "children": children,
            "parent": {
                "type": 1,
                "tagType": 0,
                "codegenNode": {
                    "type": 13,
                    "children": { "type": 20 }
                }
            },
            "context": { "scopeId": "data-v-test" }
        }));
        let expected_html = format!(
            r#"<div data-v-test>{}</div>"#,
            r#"<span class="foo" data-v-test></span>"#
                .repeat(STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
        );

        assert_eq!(
            projection["operations"],
            json!([{
                "kind": "stringifyParentCachedRange",
                "start": 0,
                "count": 1,
                "html": quote_string(&expected_html),
                "domNodes": 1
            }])
        );
    }

    #[test]
    fn stringify_static_projection_infers_nested_svg_namespace() {
        let children = vec![json!({
            "type": 1,
            "tag": "svg",
            "tagType": 0,
            "props": [{
                "type": 6,
                "name": "viewBox",
                "value": { "content": "0 0 50 50" }
            }],
            "children": (0..STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
                .map(|_| json!({
                    "type": 1,
                    "tag": "rect",
                    "tagType": 0,
                    "props": [{
                        "type": 6,
                        "name": "fill",
                        "value": { "content": "#C4C4C4" }
                    }],
                    "children": []
                }))
                .collect::<Vec<_>>()
        })];
        let projection = stringify_static_projection(&json!({
            "children": children,
            "parent": {
                "type": 1,
                "tagType": 0,
                "codegenNode": {
                    "type": 13,
                    "children": { "type": 20 }
                }
            },
            "context": {}
        }));
        let expected_html = format!(
            r#"<svg viewBox="0 0 50 50">{}</svg>"#,
            r##"<rect fill="#C4C4C4"></rect>"##.repeat(STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT)
        );

        assert_eq!(
            projection["operations"],
            json!([{
                "kind": "stringifyParentCachedRange",
                "start": 0,
                "count": 1,
                "html": quote_string(&expected_html),
                "domNodes": 1
            }])
        );
    }

    #[test]
    fn stringify_static_projection_bails_can_cache_option_values() {
        let children = vec![json!({
            "type": 1,
            "tag": "option",
            "tagType": 0,
            "ns": 0,
            "props": [{
                "type": 7,
                "name": "bind",
                "arg": { "type": 4, "content": "value", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "_imports_0",
                    "isStatic": false,
                    "constType": VUE3_CONSTANT_CAN_STRINGIFY
                }
            }],
            "children": [],
            "codegenNode": { "type": 20, "index": 0 }
        })];
        let projection = stringify_static_projection(&json!({
            "children": children,
            "context": {}
        }));

        assert_eq!(projection["operations"], json!([]));
    }
