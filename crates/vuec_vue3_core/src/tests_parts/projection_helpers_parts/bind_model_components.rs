    #[test]
    fn transform_element_props_projection_keeps_dynamic_handlers_unwrapped_for_normalize() {
        let projection = transform_element_props_projection(&json!({
            "props": [{
                "kind": "directiveProp",
                "dynamicKey": true,
                "ignoreDynamicKeyForNormalize": true,
                "valueConstant": false
            }],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(16));
        assert_eq!(projection["normalizeProps"], json!(false));
    }

    #[test]
    fn transform_bind_projection_projects_static_and_dynamic_args() {
        let static_projection = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "id", "isStatic": true, "loc": { "source": "id" } },
                "exp": { "type": 4, "content": "id", "isStatic": false, "loc": { "source": "id" } },
                "modifiers": []
            },
            "context": {}
        }));
        assert_eq!(
            static_projection["props"][0]["key"],
            json!({
                "kind": "simple",
                "content": "id",
                "isStatic": true,
                "loc": { "source": "id" }
            })
        );
        assert_eq!(
            static_projection["props"][0]["value"],
            json!({ "kind": "node", "path": "dir.exp" })
        );

        let dynamic_projection = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "id", "isStatic": false, "loc": { "source": "[id]" } },
                "exp": { "type": 4, "content": "value", "isStatic": false },
                "modifiers": []
            },
            "context": {}
        }));
        assert_eq!(
            dynamic_projection["props"][0]["key"]["content"],
            json!("id || \"\"")
        );
        assert_eq!(
            dynamic_projection["props"][0]["key"]["isStatic"],
            json!(false)
        );
    }

    #[test]
    fn transform_bind_projection_applies_camel_and_prefix_modifiers() {
        let static_camel = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "foo-bar", "isStatic": true },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "camel" }]
            },
            "context": {}
        }));
        assert_eq!(static_camel["props"][0]["key"]["content"], json!("fooBar"));

        let dynamic_camel = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "foo", "isStatic": false },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "camel" }]
            },
            "context": {}
        }));
        assert_eq!(
            dynamic_camel["props"][0]["key"]["content"],
            json!("_camelize(foo || \"\")")
        );
        assert_eq!(
            dynamic_camel["props"][0]["key"]["helpers"],
            json!(["CAMELIZE"])
        );

        let dynamic_prop = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "fooBar", "isStatic": false },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "prop" }]
            },
            "context": {}
        }));
        assert_eq!(
            dynamic_prop["props"][0]["key"]["content"],
            json!("`.${fooBar || \"\"}`")
        );

        let ssr_prop = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "fooBar", "isStatic": true },
                "exp": { "type": 4, "content": "id", "isStatic": false },
                "modifiers": [{ "content": "prop" }]
            },
            "context": { "inSSR": true }
        }));
        assert_eq!(ssr_prop["props"][0]["key"]["content"], json!("fooBar"));
    }

    #[test]
    fn transform_bind_projection_handles_compound_args_and_empty_expressions() {
        let compound = transform_bind_projection(&json!({
            "dir": {
                "arg": {
                    "type": 8,
                    "children": [
                        { "type": 4, "content": "_ctx.foo", "isStatic": false },
                        "(",
                        { "type": 4, "content": "_ctx.bar", "isStatic": false },
                        ")"
                    ],
                    "loc": { "source": "foo(bar)" }
                },
                "exp": { "type": 4, "content": "_ctx.id", "isStatic": false },
                "modifiers": [{ "content": "camel" }, { "content": "prop" }]
            },
            "context": {}
        }));
        assert_eq!(compound["props"][0]["key"]["children"][0], json!("'.' + ("));
        assert_eq!(
            compound["props"][0]["key"]["children"][1]["children"][0],
            json!({ "kind": "helperString", "helper": "CAMELIZE" })
        );
        assert_eq!(
            compound["props"][0]["key"]["children"][1]["children"][1]["children"][0],
            json!("(")
        );

        let missing = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "arg", "isStatic": true },
                "exp": { "type": 4, "content": "   ", "isStatic": false },
                "modifiers": [],
                "loc": { "source": "v-bind:arg=\"\"" }
            },
            "context": {}
        }));
        assert_eq!(missing["errors"], json!([{ "code": 34, "loc": "dir" }]));
        assert_eq!(missing["props"][0]["value"]["content"], json!(""));
        assert_eq!(missing["props"][0]["value"]["isStatic"], json!(true));

        let browser_missing = transform_bind_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "arg", "isStatic": true },
                "exp": { "type": 4, "content": "", "isStatic": false },
                "modifiers": []
            },
            "context": { "browser": true }
        }));
        assert_eq!(browser_missing["errors"], json!([]));
        assert_eq!(
            browser_missing["props"][0]["value"],
            json!({ "kind": "undefined" })
        );
    }

    #[test]
    fn transform_v_bind_shorthand_projection_expands_static_same_name_bindings() {
        let projection = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": {
                        "type": 4,
                        "content": "foo-bar",
                        "isStatic": true,
                        "loc": { "source": "foo-bar" }
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"][0],
            json!({
                "kind": "setExp",
                "index": 0,
                "exp": {
                    "kind": "simple",
                    "content": "fooBar",
                    "isStatic": false,
                    "loc": { "source": "foo-bar" }
                },
                "errors": []
            })
        );
    }

    #[test]
    fn transform_v_bind_shorthand_projection_reports_dynamic_args_and_browser_empty_exp() {
        let invalid = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "foo", "isStatic": false, "loc": { "source": "[foo]" } }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            invalid["operations"][0]["errors"],
            json!([{ "code": 53, "loc": "arg" }])
        );
        assert_eq!(invalid["operations"][0]["exp"]["content"], json!(""));
        assert_eq!(invalid["operations"][0]["exp"]["isStatic"], json!(true));

        let browser_empty = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "name", "isStatic": true, "loc": { "source": "name" } },
                    "exp": { "type": 4, "content": "  ", "isStatic": false }
                }]
            },
            "context": { "browser": true }
        }));
        assert_eq!(
            browser_empty["operations"][0]["exp"]["content"],
            json!("name")
        );

        let invalid_first_char = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "1bad", "isStatic": true }
                }]
            },
            "context": {}
        }));
        assert_eq!(invalid_first_char["operations"], json!([]));

        let unicode_first_char = transform_v_bind_shorthand_projection(&json!({
            "node": {
                "type": 1,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "éclair", "isStatic": true, "loc": { "source": "éclair" } }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            unicode_first_char["operations"][0]["exp"]["content"],
            json!("éclair")
        );
    }

    #[test]
    fn transform_on_projection_marks_setup_const_handlers_constant() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "keydown", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "bindingMetadata": { "foo": "setup-const" }
            }
        }));

        assert_eq!(
            projection["props"][0]["value"]["content"],
            json!("$setup.foo")
        );
        assert_eq!(projection["props"][0]["value"]["constType"], json!(1));
        assert_eq!(projection["props"][0]["valueConstant"], json!(true));
    }

    #[test]
    fn transform_model_projection_emits_model_value_and_update_props() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "static", "content": "modelValue" })
        );
        assert_eq!(projection["props"][0]["dynamic"], json!(true));
        assert_eq!(
            projection["props"][1]["key"],
            json!({ "kind": "static", "content": "onUpdate:modelValue" })
        );
        assert_eq!(
            projection["props"][1]["value"]["children"][0],
            json!("$event => ((")
        );
    }

    #[test]
    fn transform_model_projection_handles_dynamic_argument() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "_ctx.value",
                    "isStatic": false
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "node", "path": "dir.arg" })
        );
        assert_eq!(
            projection["props"][1]["key"],
            json!({
                "kind": "compound",
                "children": ["\"onUpdate:\" + ", { "kind": "node", "path": "dir.arg" }]
            })
        );
    }

    #[test]
    fn transform_model_projection_reports_invalid_expression_errors() {
        let no_expression = transform_model_projection(&json!({
            "dir": { "modifiers": [] },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(no_expression["errors"], json!([41]));

        let malformed = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "a + b",
                    "loc": { "source": "a + b" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([42]));
    }

    #[test]
    fn transform_model_projection_tracks_cache_and_scope_refs() {
        let cached = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.foo",
                    "loc": { "source": "foo" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {}
            }
        }));
        assert_eq!(cached["props"][1]["cache"], json!(true));
        assert_eq!(cached["props"][1]["dynamic"], json!(false));

        let scoped = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 8,
                    "loc": { "source": "foo[i]" },
                    "children": [
                        { "type": 4, "content": "_ctx.foo" },
                        "[",
                        { "type": 4, "content": "i" },
                        "]"
                    ]
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": { "i": 1 }
            }
        }));
        assert_eq!(scoped["props"][1]["cache"], json!(false));
        assert_eq!(scoped["props"][1]["dynamic"], json!(true));
    }

    #[test]
    fn transform_model_projection_generates_component_modifiers() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "foo",
                    "loc": { "source": "foo" }
                },
                "arg": {
                    "type": 4,
                    "content": "bar",
                    "isStatic": true
                },
                "modifiers": [
                    { "content": "trim" },
                    { "content": "bar-baz" }
                ]
            },
            "node": { "tagType": 1 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][2]["key"],
            json!({ "kind": "static", "content": "barModifiers" })
        );
        assert_eq!(
            projection["props"][2]["value"]["content"],
            json!("{ trim: true, \"bar-baz\": true }")
        );
    }

    #[test]
    fn transform_model_projection_marks_static_argument_hydration_event() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "foo-value",
                    "isStatic": true
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(projection["props"][1]["hydrate"], json!(true));
    }

    #[test]
    fn resolve_component_type_projection_uses_setup_bindings() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Example", "tagType": 1, "props": [] },
            "context": {
                "bindingMetadata": { "Example": "setup-maybe-ref" },
                "inline": true
            }
        }));

        assert_eq!(projection["kind"], json!("expression"));
        assert_eq!(projection["content"], json!("_unref(Example)"));
        assert_eq!(projection["helpers"], json!(["UNREF"]));
    }

    #[test]
    fn resolve_component_type_projection_handles_namespaced_props_binding() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Foo.Example", "tagType": 1, "props": [] },
            "context": {
                "bindingMetadata": { "Foo": "props" },
                "inline": false
            }
        }));

        assert_eq!(projection["kind"], json!("expression"));
        assert_eq!(
            projection["content"],
            json!("_unref($props[\"Foo\"]).Example")
        );
    }

    #[test]
    fn resolve_component_type_projection_marks_self_reference_asset() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Example", "tagType": 1, "props": [] },
            "context": { "selfName": "Example" }
        }));

        assert_eq!(projection["kind"], json!("asset"));
        assert_eq!(projection["component"], json!("Example__self"));
        assert_eq!(projection["assetId"], json!("_component_Example"));
    }

    #[test]
    fn resolve_component_type_projection_handles_dynamic_component_is() {
        let projection = resolve_component_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "component",
                "tagType": 1,
                "props": [
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "is", "isStatic": true },
                        "exp": { "type": 4, "content": "foo", "isStatic": false }
                    }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("dynamic"));
        assert_eq!(projection["helper"], json!("RESOLVE_DYNAMIC_COMPONENT"));
        assert_eq!(projection["argument"]["content"], json!("foo"));
    }

    #[test]
    fn resolve_component_type_projection_casts_vue_is_attribute() {
        let projection = resolve_component_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 1,
                "props": [
                    {
                        "type": 6,
                        "name": "is",
                        "value": { "content": "vue:foo" }
                    }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("asset"));
        assert_eq!(projection["component"], json!("foo"));
        assert_eq!(projection["assetId"], json!("_component_foo"));
    }
