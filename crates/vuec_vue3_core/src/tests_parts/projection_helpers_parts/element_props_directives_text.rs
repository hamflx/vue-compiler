    #[test]
    fn base_parse_classifies_lowercase_builtins_and_dynamic_component_as_components() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<teleport/><suspense/><keep-alive/><base-transition/><component/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let tags = root
            .children
            .iter()
            .map(|id| ast.node(*id).expect("element"))
            .map(|node| match &node.kind {
                Vue3AstKind::Element(element) => (&element.tag, element.tag_type),
                _ => panic!("expected element"),
            })
            .collect::<Vec<_>>();

        assert!(tags
            .iter()
            .all(|(_, tag_type)| *tag_type == Vue3ElementType::Component));
    }

    #[test]
    fn transform_element_props_projection_flags_class_style_and_dynamic_props() {
        let projection = transform_element_props_projection(&json!({
            "props": [
                { "kind": "directiveProp", "name": "class", "valueConstant": false },
                { "kind": "directiveProp", "name": "style", "valueConstant": false },
                { "kind": "directiveProp", "name": "foo", "valueConstant": false }
            ],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(14));
        assert_eq!(projection["dynamicPropNames"], json!(["foo"]));
        assert_eq!(projection["normalizeClass"], json!(true));
        assert_eq!(projection["normalizeStyle"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_normalizes_style_arrays() {
        let array_literal = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "style",
                    "valueConstant": true,
                    "valueStartsWithArray": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(array_literal["normalizeStyle"], json!(true));

        let merged_style = transform_element_props_projection(&json!({
            "props": [
                { "kind": "attribute", "name": "style" },
                {
                    "kind": "directiveProp",
                    "name": "style",
                    "valueConstant": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(merged_style["normalizeStyle"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_wraps_object_bind_props() {
        let projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "objectBind" }],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(16));
        assert_eq!(projection["normalizeProps"], json!(true));
        assert_eq!(projection["guardReactiveProps"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_marks_ref_and_runtime_directives_need_patch() {
        let ref_projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(ref_projection["patchFlag"], json!(512));

        let runtime_projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "runtimeDirective" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(runtime_projection["patchFlag"], json!(512));
    }

    #[test]
    fn transform_element_props_projection_marks_ref_for_in_v_for_scope() {
        let static_ref = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(static_ref["refForMarker"], json!(true));

        let dynamic_ref = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "ref",
                    "valueConstant": false
                }
            ],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(dynamic_ref["refForMarker"], json!(true));

        let object_bind = transform_element_props_projection(&json!({
            "props": [{ "kind": "objectBind" }],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(object_bind["refForMarker"], json!(true));

        let outside_for = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(outside_for["refForMarker"], json!(false));
    }

    #[test]
    fn transform_element_props_projection_forces_blocks_for_selected_props() {
        let key_projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "key",
                    "forceBlock": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(key_projection["shouldUseBlock"], json!(true));

        let vnode_hook_projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "forceBlock": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(vnode_hook_projection["shouldUseBlock"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_projects_inline_template_ref_keys() {
        let projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref", "value": "input" }],
            "context": {
                "inline": true,
                "bindingMetadata": {
                    "input": "setup-ref"
                }
            },
            "isComponent": false
        }));

        assert_eq!(
            projection["inlineTemplateRefs"],
            json!([{ "content": "input" }])
        );

        let outside_inline = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref", "value": "input" }],
            "context": {
                "bindingMetadata": {
                    "input": "setup-ref"
                }
            },
            "isComponent": false
        }));
        assert_eq!(outside_inline["inlineTemplateRefs"], json!([]));
    }

    #[test]
    fn build_directive_args_projection_keeps_runtime_directive_shape() {
        let projection = build_directive_args_projection(&json!({
            "dir": {
                "name": "baz",
                "exp": { "type": 4, "content": "y" },
                "arg": { "type": 4, "content": "arg", "isStatic": false },
                "modifiers": ["mod", "mad"]
            }
        }));

        assert_eq!(
            projection,
            json!({
                "runtime": {
                    "kind": "asset",
                    "name": "baz"
                },
                "includeExp": true,
                "includeArg": true,
                "modifiers": [
                    { "name": "mod" },
                    { "name": "mad" }
                ]
            })
        );
    }

    #[test]
    fn transform_element_children_projection_lowers_builtin_component_children() {
        let suspense = transform_element_children_projection(&json!({
            "tag": "SUSPENSE",
            "children": [
                { "type": 2, "content": "foo" }
            ]
        }));
        assert_eq!(suspense["kind"], json!("slots"));
        assert_eq!(suspense["slots"][0]["name"], json!("default"));
        assert_eq!(suspense["shouldUseBlock"], json!(true));

        let suspense_templates = transform_element_children_projection(&json!({
            "tag": "SUSPENSE",
            "children": [
                {
                    "type": 1,
                    "tag": "template",
                    "props": [
                        {
                            "name": "slot",
                            "arg": { "content": "fallback" }
                        }
                    ]
                }
            ]
        }));
        assert_eq!(suspense_templates["slots"][0]["name"], json!("fallback"));
        assert_eq!(
            suspense_templates["slots"][0]["unwrapTemplate"],
            json!(true)
        );

        let keep_alive = transform_element_children_projection(&json!({
            "tag": "KEEP_ALIVE",
            "children": [
                { "type": 1, "tag": "span" }
            ]
        }));
        assert_eq!(keep_alive["kind"], json!("children"));
        assert_eq!(keep_alive["patchFlag"], json!(1024));
        assert_eq!(keep_alive["shouldUseBlock"], json!(true));
    }

    #[test]
    fn transform_text_projection_merges_and_wraps_text_children() {
        let loc = json!({
            "start": { "offset": 0, "line": 1, "column": 1 },
            "end": { "offset": 0, "line": 1, "column": 1 },
            "source": ""
        });
        let projection = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 5, "content": { "type": 4, "content": "foo", "constType": 0 }, "loc": loc },
                    { "type": 2, "content": " bar ", "loc": loc },
                    { "type": 5, "content": { "type": 4, "content": "baz", "constType": 0 }, "loc": loc }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["operations"][0]["kind"], json!("mergeText"));
        assert_eq!(projection["operations"][0]["start"], json!(0));
        assert_eq!(projection["operations"][0]["end"], json!(2));
        assert_eq!(projection["operations"].as_array().unwrap().len(), 1);

        let wrapped = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 1, "tag": "div" },
                    { "type": 2, "content": "hello", "loc": loc },
                    { "type": 1, "tag": "div" }
                ]
            },
            "context": {}
        }));
        assert_eq!(wrapped["operations"][0]["kind"], json!("wrapTextCall"));
        assert_eq!(wrapped["operations"][0]["index"], json!(1));
        assert_eq!(wrapped["operations"][0]["includeContent"], json!(true));
    }

    #[test]
    fn transform_text_projection_honors_compat_and_ssr_boundaries() {
        let loc = json!({
            "start": { "offset": 0, "line": 1, "column": 1 },
            "end": { "offset": 0, "line": 1, "column": 1 },
            "source": ""
        });
        let template = json!({
            "type": 1,
            "tag": "template",
            "tagType": 0,
            "props": [],
            "children": [{ "type": 2, "content": "hello", "loc": loc }]
        });

        assert_eq!(
            transform_text_projection(&json!({ "node": template, "context": { "compat": false } }))
                ["operations"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        let compat_projection =
            transform_text_projection(&json!({ "node": template, "context": { "compat": true } }));
        assert_eq!(
            compat_projection["operations"][0]["kind"],
            json!("wrapTextCall")
        );

        let in_ssr_projection = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 1, "tag": "div" },
                    { "type": 5, "content": { "type": 4, "content": "foo", "constType": 0 }, "loc": loc },
                    { "type": 1, "tag": "div" }
                ]
            },
            "context": { "inSSR": true }
        }));
        assert_eq!(
            in_ssr_projection["operations"][0]["patchFlag"],
            json!("1 /* TEXT */")
        );

        let ssr_projection = transform_text_projection(&json!({
            "node": {
                "type": 0,
                "children": [
                    { "type": 1, "tag": "div" },
                    { "type": 5, "content": { "type": 4, "content": "foo", "constType": 0 }, "loc": loc },
                    { "type": 1, "tag": "div" }
                ]
            },
            "context": { "ssr": true, "inSSR": true }
        }));
        assert!(ssr_projection["operations"][0]
            .get("patchFlag")
            .is_none_or(Value::is_null));
    }

    #[test]
    fn transform_element_props_projection_marks_hydration_event_without_props_for_constants() {
        let projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "onKeydown",
                    "valueConstant": true
                }
            ],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(32));
        assert_eq!(projection["dynamicPropNames"], json!([]));
    }
