    #[test]
    fn transform_for_projection_preserves_skipped_alias_slots_and_locs() {
        let source = "<span v-for=\"( item,, index ) in items\" />";
        let exp_start = source.find("( item").unwrap();
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "( item,, index ) in items",
                    "loc": {
                        "start": { "offset": exp_start, "line": 1, "column": exp_start + 1 },
                        "end": { "offset": exp_start + "( item,, index ) in items".len(), "line": 1, "column": exp_start + "( item,, index ) in items".len() + 1 },
                        "source": "( item,, index ) in items"
                    }
                },
                "loc": { "source": "v-for=\"( item,, index ) in items\"" }
            },
            "node": { "type": 1, "tagType": 0, "children": [] },
            "context": {}
        }));

        assert_eq!(projection["parseResult"]["value"]["content"], json!("item"));
        assert!(projection["parseResult"]["key"].is_null());
        assert_eq!(
            projection["parseResult"]["index"]["content"],
            json!("index")
        );
        assert_eq!(
            projection["parseResult"]["source"]["content"],
            json!("items")
        );
        assert_eq!(
            projection["parseResult"]["index"]["loc"]["start"]["offset"],
            json!(source.find("index").unwrap())
        );
    }

    #[test]
    fn transform_for_projection_reports_missing_and_malformed_expression() {
        let missing = transform_for_projection(&json!({
            "dir": { "loc": { "source": "v-for" } },
            "node": { "type": 1, "tagType": 0 },
            "context": {}
        }));
        assert_eq!(missing["errors"], json!([{ "code": 31, "loc": "dir" }]));

        let malformed = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "item in",
                    "loc": { "start": { "offset": 0, "line": 1, "column": 1 }, "source": "item in" }
                },
                "loc": { "source": "v-for=\"item in\"" }
            },
            "node": { "type": 1, "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([{ "code": 32, "loc": "dir" }]));
    }

    #[test]
    fn transform_for_projection_prefixes_source_and_alias_defaults() {
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "({ foo = bar, baz: [qux = quux] }) in list.concat([foo])",
                    "loc": {
                        "start": { "offset": 0, "line": 1, "column": 1 },
                        "end": { "offset": 58, "line": 1, "column": 59 },
                        "source": "({ foo = bar, baz: [qux = quux] }) in list.concat([foo])"
                    }
                },
                "loc": { "source": "v-for" }
            },
            "node": { "type": 1, "tagType": 0, "children": [] },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(
            projection["parseResult"]["source"]["kind"],
            json!("compound")
        );
        assert_eq!(
            projection["parseResult"]["source"]["children"][0]["content"],
            json!("_ctx.list")
        );
        assert_eq!(
            projection["parseResult"]["value"]["kind"],
            json!("compound")
        );
        let value = &projection["parseResult"]["value"]["children"];
        assert_eq!(value[1]["content"], json!("foo"));
        assert_eq!(value[3]["content"], json!("_ctx.bar"));
        assert_eq!(value[5]["content"], json!("qux"));
        assert_eq!(value[7]["content"], json!("_ctx.quux"));
        assert_eq!(projection["locals"], json!(["foo", "qux"]));
    }

    #[test]
    fn transform_for_projection_reports_template_child_key_placement() {
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "item in items",
                    "loc": { "start": { "offset": 0, "line": 1, "column": 1 }, "source": "item in items" }
                },
                "loc": { "source": "v-for" }
            },
            "node": {
                "type": 1,
                "tagType": 3,
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "props": [{
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "key", "isStatic": true },
                        "loc": { "source": ":key=\"item.id\"" }
                    }]
                }]
            },
            "context": {}
        }));
        assert_eq!(
            projection["templateKeyErrors"],
            json!([{ "code": 33, "loc": { "source": ":key=\"item.id\"" } }])
        );
    }

    #[test]
    fn build_slots_projection_tracks_slot_locals_and_dynamic_slots() {
        let projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "slot",
                    "exp": {
                        "type": 8,
                        "children": [
                            "{ ",
                            { "type": 4, "content": "foo", "isStatic": false },
                            " }"
                        ],
                        "loc": { "source": "{ foo }" }
                    }
                }],
                "children": [
                    { "type": 5, "content": { "type": 4, "content": "foo", "isStatic": false } }
                ],
                "loc": { "source": "<Comp/>" }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(
            projection["properties"][0]["key"]["content"],
            json!("default")
        );
        assert_eq!(projection["properties"][0]["indices"], json!([0]));
        assert_eq!(projection["hasDynamicSlots"], json!(false));

        let tracking = track_slot_scopes_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "slot",
                    "exp": {
                        "type": 8,
                        "children": [
                            "{ ",
                            { "type": 4, "content": "foo", "isStatic": false },
                            " }"
                        ],
                        "loc": { "source": "{ foo }" }
                    }
                }]
            }
        }));
        assert_eq!(tracking["locals"], json!(["foo"]));
    }

    #[test]
    fn build_slots_projection_lowers_if_and_for_dynamic_slots() {
        let if_projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [],
                "children": [{
                    "type": 1,
                    "tag": "template",
                    "tagType": 3,
                    "props": [
                        { "type": 7, "name": "slot", "arg": { "type": 4, "content": "one", "isStatic": true }, "loc": { "source": "#one" } },
                        { "type": 7, "name": "if", "exp": { "type": 4, "content": "_ctx.ok", "isStatic": false }, "loc": { "source": "v-if=\"ok\"" } }
                    ],
                    "children": [{ "type": 2, "content": "hello" }]
                }]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));
        assert_eq!(
            if_projection["dynamicSlots"][0]["kind"],
            json!("conditional")
        );
        assert_eq!(
            if_projection["dynamicSlots"][0]["test"]["content"],
            json!("_ctx.ok")
        );

        let for_projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [],
                "children": [{
                    "type": 1,
                    "tag": "template",
                    "tagType": 3,
                    "props": [
                        { "type": 7, "name": "slot", "arg": { "type": 4, "content": "name", "isStatic": false }, "loc": { "source": "#[name]" } },
                        {
                            "type": 7,
                            "name": "for",
                            "exp": { "type": 4, "content": "name in list", "loc": { "source": "name in list", "start": { "offset": 0, "line": 1, "column": 1 } } },
                            "forParseResult": {
                                "source": { "type": 4, "content": "_ctx.list", "isStatic": false },
                                "value": { "type": 4, "content": "name", "isStatic": false },
                                "key": null,
                                "index": null
                            }
                        }
                    ],
                    "children": [{ "type": 5, "content": { "type": 4, "content": "name", "isStatic": false } }]
                }]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));
        assert_eq!(for_projection["dynamicSlots"][0]["kind"], json!("for"));
        assert_eq!(
            for_projection["dynamicSlots"][0]["source"]["content"],
            json!("_ctx.list")
        );
        assert_eq!(
            for_projection["dynamicSlots"][0]["slot"]["name"]["content"],
            json!("name")
        );
    }

    #[test]
    fn transform_slot_outlet_projection_projects_name_props_and_codegen_shape() {
        let named = transform_slot_outlet_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 2,
                "props": [
                    { "type": 6, "name": "name", "value": { "content": "foo" } },
                    { "type": 6, "name": "foo-bar", "value": { "content": "baz" } },
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "qux-kebab", "isStatic": true },
                        "exp": { "type": 4, "content": "qux", "isStatic": false }
                    }
                ],
                "children": [{ "type": 2, "content": "fallback" }],
                "loc": { "source": "<slot/>" }
            },
            "context": { "scopeId": "data-v-test", "slotted": false }
        }));

        assert_eq!(named["transform"], json!(true));
        assert_eq!(
            named["process"]["slotName"],
            json!({ "kind": "literal", "value": "\"foo\"" })
        );
        assert_eq!(named["process"]["nonNameProps"], json!([1, 2]));
        assert_eq!(
            named["process"]["mutations"],
            json!([
                { "kind": "setPropName", "index": 1, "name": "fooBar" },
                { "kind": "setDirectiveArgContent", "index": 2, "content": "quxKebab" }
            ])
        );
        assert_eq!(named["codegen"]["expectedLen"], json!(5));
        assert_eq!(named["codegen"]["slots"], json!("$slots"));
    }

    #[test]
    fn transform_slot_outlet_projection_handles_dynamic_and_same_name_slots() {
        let dynamic = transform_slot_outlet_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 2,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "name", "isStatic": true },
                    "exp": { "type": 4, "content": "foo", "isStatic": false }
                }],
                "children": []
            },
            "context": {}
        }));
        assert_eq!(
            dynamic["process"]["slotName"],
            json!({ "kind": "node", "path": "props", "index": 0, "field": "exp" })
        );
        assert_eq!(dynamic["codegen"]["expectedLen"], json!(2));

        let shorthand = transform_slot_outlet_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 2,
                "props": [{
                    "type": 7,
                    "name": "bind",
                    "arg": { "type": 4, "content": "name", "isStatic": true, "loc": { "source": "name" } }
                }],
                "children": []
            },
            "context": { "prefixIdentifiers": true, "bindingMetadata": {} }
        }));
        assert_eq!(
            shorthand["process"]["mutations"][0],
            json!({
                "kind": "setDirectiveExp",
                "index": 0,
                "value": {
                    "kind": "simple",
                    "content": "_ctx.name",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "name" },
                    "helpers": []
                }
            })
        );
        assert_eq!(
            shorthand["process"]["slotName"],
            json!({ "kind": "node", "path": "props", "index": 0, "field": "exp" })
        );
        assert_eq!(shorthand["codegen"]["slots"], json!("_ctx.$slots"));
    }

    #[test]
    fn transform_on_projection_projects_dynamic_event_key_and_prefixes_handler() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "_ctx.event", "isStatic": false },
                "exp": { "type": 4, "content": "handler", "loc": { "source": "handler" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({
                "kind": "compound",
                "children": [
                    { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
                    { "kind": "node", "path": "dir.arg" },
                    ")"
                ]
            })
        );
        assert_eq!(
            projection["props"][0]["value"]["content"],
            json!("_ctx.handler")
        );
        assert_eq!(projection["props"][0]["dynamicKey"], json!(true));
        assert_eq!(
            projection["props"][0]["ignoreDynamicKeyForNormalize"],
            json!(true)
        );
    }

    #[test]
    fn transform_on_projection_wraps_inline_statements_and_caches_members() {
        let inline = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo($event)", "loc": { "source": "foo($event)" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(inline["props"][0]["cache"], json!(true));
        assert_eq!(
            inline["props"][0]["value"]["children"][0],
            json!("$event => (")
        );

        let member = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(member["props"][0]["cache"], json!(true));
        assert_eq!(
            member["props"][0]["value"]["children"][1]["content"],
            json!("_ctx.foo && _ctx.foo(...args)")
        );

        let component_member = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 1 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(component_member["props"][0]["cache"], json!(false));
    }

    #[test]
    fn transform_on_projection_rewrites_inline_assignment_bindings() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "maybe = count; --lett",
                    "loc": { "source": "maybe = count; --lett" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "inline": true,
                "identifiers": {},
                "bindingMetadata": {
                    "count": "setup-ref",
                    "maybe": "setup-maybe-ref",
                    "lett": "setup-let"
                }
            }
        }));

        let code = projection_code(&projection["props"][0]["value"]);
        assert!(
            code.contains("maybe.value = count.value; _isRef(lett) ? --lett.value : --lett"),
            "{code}"
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["helpers"],
            json!(["IS_REF"])
        );
    }

    #[test]
    fn transform_on_projection_rewrites_function_expression_body_refs() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "async function () { await foo() } ",
                    "loc": { "source": "async function () { await foo() } " }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(projection["props"][0]["cache"], json!(true));
        assert_eq!(
            projection["props"][0]["value"]["children"][0],
            json!("async function () { await ")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["content"],
            json!("_ctx.foo")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][2],
            json!("() } ")
        );
    }

    #[test]
    fn transform_on_projection_keeps_update_operator_child() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": {
                    "type": 4,
                    "content": "foo++",
                    "loc": { "source": "foo++" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(projection["props"][0]["cache"], json!(true));
        assert_eq!(
            projection["props"][0]["value"]["children"][0],
            json!("$event => (")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["children"][0]["content"],
            json!("_ctx.foo")
        );
        assert_eq!(
            projection["props"][0]["value"]["children"][1]["children"][1],
            json!("++")
        );
        assert_eq!(projection["props"][0]["value"]["children"][2], json!(")"));
    }
