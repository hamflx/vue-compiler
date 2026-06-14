    #[test]
    fn transform_style_projection_rewrites_static_style() {
        let projection = transform_style_projection(&json!({
            "node": {
                "props": [
                    {
                        "type": 6,
                        "name": "style",
                        "value": {
                            "content": "color: green; background: url(a;b); /* x */ margin: 0"
                        }
                    }
                ]
            }
        }));

        assert_eq!(projection["replacements"][0]["index"], json!(0));
        assert_eq!(
            projection["replacements"][0]["expression"],
            json!("{\"color\":\"green\",\"background\":\"url(a;b)\",\"margin\":\"0\"}")
        );
    }

    #[test]
    fn transform_v_html_projection_reports_children_and_clears_them() {
        let projection = transform_v_html_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "raw",
                    "isStatic": false,
                    "constType": 0
                },
                "loc": { "source": "v-html=\"raw\"" }
            },
            "node": {
                "children": [
                    { "type": 2, "content": "old" }
                ]
            }
        }));

        assert_eq!(projection["clearChildren"], json!(true));
        assert_eq!(projection["errors"][0]["code"], json!(55));
        assert_eq!(projection["errors"][0]["loc"], json!("dir"));
        assert_eq!(projection["props"][0]["key"], json!("innerHTML"));
        assert_eq!(projection["props"][0]["keyLoc"], json!("dir"));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("node"));
        assert_eq!(projection["props"][0]["value"]["path"], json!("dir.exp"));
    }

    #[test]
    fn transform_v_html_projection_reports_missing_expression() {
        let projection = transform_v_html_projection(&json!({
            "dir": {
                "loc": { "source": "v-html" }
            },
            "node": {
                "children": []
            }
        }));

        assert_eq!(projection["clearChildren"], json!(false));
        assert_eq!(projection["errors"][0]["code"], json!(54));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("simple"));
        assert_eq!(projection["props"][0]["value"]["content"], json!(""));
        assert_eq!(projection["props"][0]["value"]["isStatic"], json!(true));
    }

    #[test]
    fn transform_v_text_projection_wraps_dynamic_expression() {
        let projection = transform_v_text_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "msg",
                    "isStatic": false,
                    "constType": 0
                },
                "loc": { "source": "v-text=\"msg\"" }
            },
            "node": {
                "children": []
            }
        }));

        assert_eq!(projection["errors"].as_array().unwrap().len(), 0);
        assert_eq!(projection["props"][0]["key"], json!("textContent"));
        assert!(projection["props"][0]["keyLoc"].is_null());
        assert_eq!(
            projection["props"][0]["value"]["kind"],
            json!("displayString")
        );
        assert_eq!(
            projection["props"][0]["value"]["argument"]["path"],
            json!("dir.exp")
        );
    }

    #[test]
    fn transform_v_text_projection_keeps_constant_expression() {
        let projection = transform_v_text_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "'hi'",
                    "isStatic": false,
                    "constType": 3
                },
                "loc": { "source": "v-text=\"'hi'\"" }
            },
            "node": {
                "children": [
                    { "type": 2, "content": "old" }
                ]
            }
        }));

        assert_eq!(projection["clearChildren"], json!(true));
        assert_eq!(projection["errors"][0]["code"], json!(57));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("node"));
        assert_eq!(projection["props"][0]["value"]["path"], json!("dir.exp"));
    }

    #[test]
    fn transform_show_projection_returns_runtime_helper() {
        let projection = transform_show_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "ok",
                    "isStatic": false,
                    "constType": 0
                },
                "loc": { "source": "v-show=\"ok\"" }
            }
        }));

        assert_eq!(projection["props"].as_array().unwrap().len(), 0);
        assert_eq!(projection["errors"].as_array().unwrap().len(), 0);
        assert_eq!(projection["needRuntime"], json!("V_SHOW"));
    }

    #[test]
    fn transform_show_projection_reports_missing_expression() {
        let projection = transform_show_projection(&json!({
            "dir": {
                "loc": { "source": "v-show" }
            }
        }));

        assert_eq!(projection["props"].as_array().unwrap().len(), 0);
        assert_eq!(projection["errors"][0]["code"], json!(62));
        assert_eq!(projection["errors"][0]["loc"], json!("dir"));
        assert_eq!(projection["needRuntime"], json!("V_SHOW"));
    }

    #[test]
    fn transform_on_projection_wraps_non_key_modifiers() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "click",
                    "isStatic": true,
                    "loc": { "source": "click" }
                },
                "exp": {
                    "type": 4,
                    "content": "test",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "test" }
                },
                "modifiers": [{ "content": "stop" }, { "content": "prevent" }],
                "loc": { "source": "@click.stop.prevent=\"test\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(projection["props"][0]["key"]["content"], json!("onClick"));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("call"));
        assert_eq!(
            projection["props"][0]["value"]["callee"],
            json!("V_ON_WITH_MODIFIERS")
        );
        assert_eq!(
            projection["props"][0]["value"]["arguments"][1],
            json!("[\"stop\",\"prevent\"]")
        );
    }

    #[test]
    fn transform_on_projection_wraps_key_and_option_modifiers() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "keydown",
                    "isStatic": true,
                    "loc": { "source": "keydown" }
                },
                "exp": {
                    "type": 4,
                    "content": "test",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "test" }
                },
                "modifiers": [
                    { "content": "stop" },
                    { "content": "capture" },
                    { "content": "ctrl" },
                    { "content": "a" }
                ],
                "loc": { "source": "@keydown.stop.capture.ctrl.a=\"test\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(
            projection["props"][0]["key"]["content"],
            json!("onKeydownCapture")
        );
        let value = &projection["props"][0]["value"];
        assert_eq!(value["callee"], json!("V_ON_WITH_KEYS"));
        assert_eq!(value["arguments"][1], json!("[\"a\"]"));
        assert_eq!(
            value["arguments"][0]["callee"],
            json!("V_ON_WITH_MODIFIERS")
        );
        assert_eq!(
            value["arguments"][0]["arguments"][1],
            json!("[\"stop\",\"ctrl\"]")
        );
    }

    #[test]
    fn transform_on_projection_rewrites_click_right() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "click",
                    "isStatic": true,
                    "loc": { "source": "click" }
                },
                "exp": {
                    "type": 4,
                    "content": "test",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "test" }
                },
                "modifiers": [{ "content": "right" }],
                "loc": { "source": "@click.right=\"test\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][0]["key"]["content"],
            json!("onContextmenu")
        );
        assert_eq!(
            projection["props"][0]["value"]["callee"],
            json!("V_ON_WITH_MODIFIERS")
        );
        assert_eq!(
            projection["props"][0]["value"]["arguments"][1],
            json!("[\"right\"]")
        );
    }

    #[test]
    fn transform_on_projection_preserves_constant_handler_metadata() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "keydown",
                    "isStatic": true,
                    "loc": { "source": "keydown" }
                },
                "exp": {
                    "type": 4,
                    "content": "foo",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "foo" }
                },
                "modifiers": [{ "content": "up" }],
                "loc": { "source": "@keydown.up=\"foo\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "bindingMetadata": { "foo": "setup-const" }
            }
        }));

        assert_eq!(
            projection["props"][0]["value"]["callee"],
            json!("V_ON_WITH_KEYS")
        );
        assert_eq!(projection["props"][0]["valueConstant"], json!(true));
    }

    #[test]
    fn compile_includes_core_structural_parser_diagnostics() {
        let result = compile(
            TemplateSource {
                filename: "bad.vue".into(),
                source: "<div><span></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions::default(),
        );

        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "24"
                && diagnostic.message == "Element is missing end tag."));
    }
