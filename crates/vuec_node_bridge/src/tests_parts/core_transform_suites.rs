    use crate::*;
    use std::collections::BTreeSet;
    use vuec_bridge_registry::bridge_commands;

    #[test]
    fn process_expression_left_deep_parent_sensitive_input_stays_bounded() {
        const CHILD_ENV: &str = "VUEC_PROCESS_EXPRESSION_LEFT_DEEP_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            let chain = vec!["a"; 2_000].join("+");
            let content = format!("foo({chain})");
            assert!(content.len() < 4 * 1024);
            let projection = dispatch(
                "vue3.core.processExpression",
                json!({
                    "node": {
                        "type": 4,
                        "content": content,
                        "isStatic": false,
                        "loc": {
                            "start": { "offset": 0, "line": 1, "column": 1 },
                            "end": {
                                "offset": content.len(),
                                "line": 1,
                                "column": content.len() + 1
                            },
                            "source": content
                        }
                    },
                    "context": {
                        "prefixIdentifiers": true,
                        "identifiers": {},
                        "bindingMetadata": {}
                    }
                }),
            )
            .expect("process bounded left-deep expression");
            assert_ne!(projection["kind"], json!("error"));
            return;
        }

        let output = std::process::Command::new(
            std::env::current_exe().expect("locate bridge test executable"),
        )
        .args([
            "--exact",
            "tests::process_expression_left_deep_parent_sensitive_input_stays_bounded",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .output()
        .expect("spawn bounded expression child process");
        assert!(
            output.status.success(),
            "left-deep expression child failed with {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn first_projected_prop(source_text: &str) -> Value {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: source_text.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let ast = Vue3Dialect::base_parse(source.clone(), &options);
        let projected = vue3_parse_value(
            &ast,
            &source.source,
            source.base_offset,
            false,
            &options,
            false,
        );
        projected["children"][0]["props"][0].clone()
    }

    fn json_array_contains(value: &Value, expected: &str) -> bool {
        value
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == expected))
    }

    fn dispatch_source_commands() -> BTreeSet<&'static str> {
        let dispatch = include_str!("../dispatch.rs");
        let mut commands = BTreeSet::new();
        for line in dispatch.lines() {
            let line = line.trim_start();
            if !line.starts_with('"') {
                continue;
            }
            for (index, part) in line.split('"').enumerate() {
                if index % 2 == 1 && part.contains('.') {
                    commands.insert(part);
                }
            }
        }
        commands
    }

    #[test]
    fn bridge_dispatch_commands_are_registered() {
        let dispatch_commands = dispatch_source_commands();
        let registry_commands: BTreeSet<_> = bridge_commands()
            .iter()
            .map(|command| command.name)
            .collect();
        let missing: Vec<_> = dispatch_commands
            .difference(&registry_commands)
            .copied()
            .collect();
        let stale: Vec<_> = registry_commands
            .difference(&dispatch_commands)
            .copied()
            .collect();

        assert!(
            missing.is_empty(),
            "dispatch command(s) missing from registry: {missing:?}"
        );
        assert!(
            stale.is_empty(),
            "registry command(s) missing from dispatch: {stale:?}"
        );
    }

    #[test]
    fn vue3_directive_projection_preserves_dynamic_arg_exp_and_modifier_exactness() {
        let directive = first_projected_prop(r#"<div v-bind:[foo].camel="  bar  "/>"#);

        assert_eq!(directive["name"], json!("bind"));
        assert_eq!(directive["rawName"], json!("v-bind:[foo].camel"));
        assert_eq!(directive["arg"]["content"], json!("foo"));
        assert_eq!(directive["arg"]["isStatic"], json!(false));
        assert_eq!(directive["arg"]["loc"]["source"], json!("[foo]"));
        assert_eq!(directive["exp"]["content"], json!("  bar  "));
        assert_eq!(directive["exp"]["loc"]["source"], json!("  bar  "));
        assert_eq!(directive["modifiers"][0]["content"], json!("camel"));
        assert_eq!(directive["modifiers"][0]["isStatic"], json!(true));
        assert_eq!(directive["modifiers"][0]["loc"]["source"], json!("camel"));
    }

    #[test]
    fn vue3_directive_projection_preserves_prop_shorthand_synthetic_modifier_shape() {
        let directive = first_projected_prop(r#"<div .foo="bar"/>"#);

        assert_eq!(directive["name"], json!("bind"));
        assert_eq!(directive["rawName"], json!(".foo"));
        assert_eq!(directive["arg"]["content"], json!("foo"));
        assert_eq!(directive["arg"]["isStatic"], json!(true));
        assert_eq!(directive["arg"]["loc"]["source"], json!("foo"));
        assert_eq!(directive["exp"]["content"], json!("bar"));
        assert_eq!(directive["modifiers"][0]["content"], json!("prop"));
        assert_eq!(directive["modifiers"][0]["isStatic"], json!(false));
        assert_eq!(directive["modifiers"][0]["loc"]["source"], json!(""));
    }

    #[test]
    fn vue3_transform_slot_outlet_suite_materializes_bind_shorthand_name() {
        let transformed = dispatch(
            "vue3.core.transformSlotOutletSuite",
            json!({ "source": "<slot :name />", "options": {} }),
        )
        .expect("transformSlotOutlet suite");

        let arguments = transformed["children"][0]["codegenNode"]["arguments"]
            .as_array()
            .expect("call arguments");
        assert_eq!(arguments[0], json!("$slots"));
        assert_eq!(arguments[1]["type"], json!(4));
        assert_eq!(arguments[1]["content"], json!("name"));
        assert_eq!(arguments[1]["isStatic"], json!(false));
        assert!(arguments[1].get("kind").is_none());
    }

    #[test]
    fn vue3_transform_element_suite_materializes_plain_props() {
        let transformed = dispatch(
            "vue3.core.transformElementSuite",
            json!({ "source": r#"<div><div id="foo" class="bar"/></div>"#, "options": {} }),
        )
        .expect("transformElement suite plain props");
        let props = &transformed["node"]["props"]["properties"];

        assert_eq!(transformed["node"]["tag"], json!("\"div\""));
        assert_eq!(props[0]["key"]["content"], json!("id"));
        assert_eq!(props[0]["value"]["content"], json!("foo"));
        assert_eq!(props[1]["key"]["content"], json!("class"));
        assert_eq!(props[1]["value"]["content"], json!("bar"));
        assert!(transformed["node"]["children"].is_null());
    }

    #[test]
    fn vue3_transform_element_suite_resolves_assets_and_setup_bindings() {
        let asset = dispatch(
            "vue3.core.transformElementSuite",
            json!({ "source": "<div><Foo/></div>", "options": {} }),
        )
        .expect("transformElement suite asset component");

        assert_eq!(asset["node"]["tag"], json!("_component_Foo"));
        assert_eq!(asset["root"]["components"], json!(["Foo"]));
        assert!(json_array_contains(
            &asset["root"]["helpers"],
            "RESOLVE_COMPONENT"
        ));

        let setup = dispatch(
            "vue3.core.transformElementSuite",
            json!({
                "source": "<div><Example/></div>",
                "options": { "bindingMetadata": { "Example": "setup-maybe-ref" } },
            }),
        )
        .expect("transformElement suite setup component");

        assert_eq!(setup["node"]["tag"], json!("$setup[\"Example\"]"));
        assert_eq!(setup["root"]["components"], json!([]));
        assert!(!json_array_contains(
            &setup["root"]["helpers"],
            "RESOLVE_COMPONENT"
        ));

        let inline = dispatch(
            "vue3.core.transformElementSuite",
            json!({
                "source": "<div><Example/></div>",
                "options": {
                    "inline": true,
                    "bindingMetadata": { "Example": "setup-maybe-ref" }
                },
            }),
        )
        .expect("transformElement suite inline setup component");

        assert_eq!(inline["node"]["tag"], json!("_unref(Example)"));
        assert_eq!(inline["root"]["components"], json!([]));
        assert!(json_array_contains(&inline["root"]["helpers"], "UNREF"));
    }

    #[test]
    fn vue3_transform_element_suite_resolves_dynamic_component_helper() {
        let transformed = dispatch(
            "vue3.core.transformElementSuite",
            json!({
                "source": r#"<div><component :is="foo" /></div>"#,
                "options": { "transformBind": true },
            }),
        )
        .expect("transformElement suite dynamic component");

        assert_eq!(transformed["node"]["isBlock"], json!(true));
        assert_eq!(
            transformed["node"]["tag"]["callee"],
            json!("RESOLVE_DYNAMIC_COMPONENT")
        );
        assert_eq!(
            transformed["node"]["tag"]["arguments"][0]["content"],
            json!("foo")
        );
        assert!(json_array_contains(
            &transformed["root"]["helpers"],
            "RESOLVE_DYNAMIC_COMPONENT"
        ));
    }

    #[test]
    fn vue3_transform_element_suite_keeps_keep_alive_children() {
        let transformed = dispatch(
            "vue3.core.transformElementSuite",
            json!({ "source": "<div><KeepAlive><span /></KeepAlive></div>", "options": {} }),
        )
        .expect("transformElement suite KeepAlive");

        assert_eq!(transformed["node"]["tag"], json!("KEEP_ALIVE"));
        assert_eq!(transformed["node"]["isBlock"], json!(true));
        assert_eq!(transformed["node"]["patchFlag"], json!(1024));
        assert_eq!(transformed["node"]["children"][0]["tag"], json!("span"));
        assert!(json_array_contains(
            &transformed["root"]["helpers"],
            "KEEP_ALIVE"
        ));
    }

    #[test]
    fn vue3_transform_element_suite_applies_serialized_noop_directive_transform() {
        let transformed = dispatch(
            "vue3.core.transformElementSuite",
            json!({
                "source": "<div><div v-noop/></div>",
                "options": { "noopDirectiveTransforms": ["noop"] },
            }),
        )
        .expect("transformElement suite noop directive");

        assert!(transformed["node"]["props"].is_null());
        assert!(transformed["node"]["directives"].is_null());
        assert!(transformed["node"]["patchFlag"].is_null());
        assert_eq!(transformed["root"]["directives"], json!([]));
        assert!(!json_array_contains(
            &transformed["root"]["helpers"],
            "RESOLVE_DIRECTIVE"
        ));
        assert!(!json_array_contains(
            &transformed["root"]["helpers"],
            "WITH_DIRECTIVES"
        ));
    }

    #[test]
    fn vue3_transform_suite_materializes_root_codegen_contract() {
        let empty = dispatch(
            "vue3.core.transformSuite",
            json!({ "source": "", "options": {} }),
        )
        .expect("transform suite empty root");
        assert!(empty["codegenNode"].is_null());
        assert_eq!(empty["helpers"], json!([]));

        let slot = dispatch(
            "vue3.core.transformSuite",
            json!({ "source": "<slot/>", "options": {} }),
        )
        .expect("transform suite slot root");
        assert_eq!(slot["codegenNode"]["type"], json!(1));
        assert_eq!(
            slot["codegenNode"]["codegenNode"]["callee"],
            json!("RENDER_SLOT")
        );
        assert!(json_array_contains(&slot["helpers"], "RENDER_SLOT"));

        let for_root = dispatch(
            "vue3.core.transformSuite",
            json!({ "source": r#"<div v-for="i in list" />"#, "options": {} }),
        )
        .expect("transform suite for root");
        assert_eq!(for_root["codegenNode"]["type"], json!(11));
        assert_eq!(for_root["codegenNode"]["source"]["content"], json!("list"));
        assert_eq!(
            for_root["codegenNode"]["codegenNode"]["children"]["callee"],
            json!("RENDER_LIST")
        );
        assert!(json_array_contains(&for_root["helpers"], "RENDER_LIST"));

        let comments = dispatch(
            "vue3.core.transformSuite",
            json!({ "source": "<!--foo--><div/><!--bar-->", "options": {} }),
        )
        .expect("transform suite comments root");
        assert_eq!(comments["codegenNode"]["type"], json!(13));
        assert_eq!(comments["codegenNode"]["tag"], json!("FRAGMENT"));
        assert_eq!(comments["codegenNode"]["patchFlag"], json!(2112));
        assert_eq!(comments["codegenNode"]["children"][0]["type"], json!(3));
        assert_eq!(comments["codegenNode"]["children"][1]["tag"], json!("div"));
        assert_eq!(comments["codegenNode"]["children"][2]["type"], json!(3));
        assert!(json_array_contains(&comments["helpers"], "CREATE_COMMENT"));
        assert!(json_array_contains(&comments["helpers"], "FRAGMENT"));
    }

    #[test]
    fn vue3_transform_slot_suite_materializes_default_and_dynamic_slots() {
        let transformed = dispatch(
            "vue3.core.transformSlotSuite",
            json!({ "source": "<Comp><div/></Comp>", "options": { "prefixIdentifiers": true } }),
        )
        .expect("transformSlot suite default slot");
        let slots = &transformed["slots"];
        assert_eq!(slots["type"], json!(15));
        assert_eq!(slots["properties"][0]["key"]["content"], json!("default"));
        assert_eq!(slots["properties"][0]["value"]["type"], json!(18));
        assert!(slots["properties"][0]["value"]["params"].is_null());
        assert_eq!(
            slots["properties"][0]["value"]["returns"][0]["tag"],
            json!("div")
        );
        assert_eq!(
            slots["properties"][1]["value"]["content"],
            json!("1 /* STABLE */")
        );
        assert_eq!(
            transformed["root"]["helpers"],
            json!([
                "CREATE_ELEMENT_VNODE",
                "RESOLVE_COMPONENT",
                "WITH_CTX",
                "OPEN_BLOCK",
                "CREATE_BLOCK"
            ])
        );

        let dynamic = dispatch(
            "vue3.core.transformSlotSuite",
            json!({ "source": r#"<Comp><template #one v-if="ok">hello</template></Comp>"#, "options": {} }),
        )
        .expect("transformSlot suite dynamic slot");
        let dynamic_slots = &dynamic["slots"];
        assert_eq!(dynamic_slots["type"], json!(14));
        assert_eq!(dynamic_slots["callee"], json!("CREATE_SLOTS"));
        assert_eq!(
            dynamic_slots["arguments"][0]["properties"][0]["value"]["content"],
            json!("2 /* DYNAMIC */")
        );
        let branch = &dynamic_slots["arguments"][1]["elements"][0];
        assert_eq!(branch["type"], json!(19));
        assert_eq!(branch["test"]["content"], json!("ok"));
        assert_eq!(
            branch["consequent"]["properties"][0]["value"]["content"],
            json!("one")
        );
        assert_eq!(
            branch["consequent"]["properties"][1]["value"]["returns"][0]["content"],
            json!("hello")
        );
        assert_eq!(
            branch["consequent"]["properties"][2]["value"]["content"],
            json!("0")
        );
        assert_eq!(
            dynamic["root"]["children"][0]["codegenNode"]["patchFlag"],
            json!(1024)
        );
    }

    #[test]
    fn vue3_transform_slot_suite_tracks_nested_scope_and_forwarded_flag() {
        let scoped = dispatch(
            "vue3.core.transformSlotSuite",
            json!({
                "source": r#"<Comp><template #default="{ foo }"><Inner v-slot="{ bar }">{{ foo }}{{ bar }}{{ baz }}</Inner></template></Comp>"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformSlot suite nested scope");
        assert_eq!(scoped["root"]["components"], json!(["Inner", "Comp"]));
        let default_returns = &scoped["slots"]["properties"][0]["value"]["returns"];
        let inner = &default_returns[0];
        assert_eq!(inner["tag"], json!("Inner"));
        assert_eq!(
            inner["codegenNode"]["children"]["properties"][0]["value"]["returns"][0]["content"]
                ["content"],
            json!("foo")
        );
        assert_eq!(
            inner["codegenNode"]["children"]["properties"][0]["value"]["returns"][1]["content"]
                ["content"],
            json!("bar")
        );
        assert_eq!(
            inner["codegenNode"]["children"]["properties"][0]["value"]["returns"][2]["content"]
                ["content"],
            json!("_ctx.baz")
        );
        assert_eq!(inner["codegenNode"]["patchFlag"], json!(1024));

        let forwarded = dispatch(
            "vue3.core.transformSlotSuite",
            json!({ "source": "<Comp><slot/></Comp>", "options": {} }),
        )
        .expect("transformSlot suite forwarded slot");
        assert_eq!(
            forwarded["slots"]["properties"][1]["value"]["content"],
            json!("3 /* FORWARDED */")
        );
    }

    #[test]
    fn vue3_transform_slot_suite_reports_slot_errors() {
        let transformed = dispatch(
            "vue3.core.transformSlotSuite",
            json!({ "source": "<Comp><template #default>foo</template>bar</Comp>", "options": {} }),
        )
        .expect("transformSlot suite slot errors");

        assert_eq!(transformed["root"]["__vuecErrors"][0]["code"], json!(39));
        assert_eq!(
            transformed["root"]["__vuecErrors"][0]["loc"]["source"],
            json!("bar")
        );
    }

    #[test]
    fn vue3_cache_static_suite_caches_children_arrays_and_slot_returns() {
        let child_array = dispatch(
            "vue3.core.cacheStaticSuite",
            json!({ "source": r#"<div><span class="inline">hello</span></div>"#, "options": {} }),
        )
        .expect("cacheStatic suite child array");
        assert_eq!(child_array["cached"].as_array().map(Vec::len), Some(1));
        assert_eq!(child_array["codegenNode"]["children"]["type"], json!(20));
        assert_eq!(
            child_array["codegenNode"]["children"]["value"]["elements"][0]["codegenNode"]
                ["patchFlag"],
            json!(-1)
        );

        let slot_array = dispatch(
            "vue3.core.cacheStaticSuite",
            json!({ "source": "<Foo><span/><span/></Foo>", "options": {} }),
        )
        .expect("cacheStatic suite slot array");
        assert_eq!(slot_array["cached"].as_array().map(Vec::len), Some(1));
        let returns = &slot_array["codegenNode"]["children"]["properties"][0]["value"]["returns"];
        assert_eq!(returns["type"], json!(20));
        assert_eq!(returns["needArraySpread"], json!(true));
        assert_eq!(
            returns["value"]["elements"][0]["codegenNode"]["patchFlag"],
            json!(-1)
        );
    }

    #[test]
    fn vue3_cache_static_suite_hoists_props_and_syncs_codegen_refs() {
        let class_hoist = dispatch(
            "vue3.core.cacheStaticSuite",
            json!({
                "source": r#"<div><span :class="{ foo: true }">{{ bar }}</span></div>"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("cacheStatic suite class hoist");
        assert_eq!(class_hoist["hoists"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            class_hoist["hoists"][0]["properties"][0]["value"]["callee"],
            json!("NORMALIZE_CLASS")
        );
        assert_eq!(
            class_hoist["codegenNode"]["children"][0]["codegenNode"]["props"]["content"],
            json!("_hoisted_1")
        );
        assert!(class_hoist["helpers"]
            .as_array()
            .expect("helpers")
            .iter()
            .any(|helper| helper == "NORMALIZE_CLASS"));

        let dynamic_props = dispatch(
            "vue3.core.cacheStaticSuite",
            json!({ "source": r#"<div><div :id="foo"/></div>"#, "options": {} }),
        )
        .expect("cacheStatic suite dynamic props hoist");
        assert_eq!(dynamic_props["hoists"], json!(["[\"id\"]"]));
        assert_eq!(
            dynamic_props["children"][0]["children"][0]["codegenNode"]["dynamicProps"]["content"],
            json!("_hoisted_1")
        );

        let if_codegen = dispatch(
            "vue3.core.cacheStaticSuite",
            json!({ "source": r#"<div><div v-if="ok" id="foo"><span/></div></div>"#, "options": {} }),
        )
        .expect("cacheStatic suite if sync");
        assert_eq!(if_codegen["cached"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            if_codegen["children"][0]["children"][0]["codegenNode"]["consequent"]["props"]
                ["content"],
            json!("_hoisted_1")
        );
        assert_eq!(
            if_codegen["children"][0]["children"][0]["codegenNode"]["consequent"]["children"]
                ["type"],
            json!(20)
        );

        let for_codegen = dispatch(
            "vue3.core.cacheStaticSuite",
            json!({ "source": r#"<div><div v-for="i in list" id="foo"><span/></div></div>"#, "options": {} }),
        )
        .expect("cacheStatic suite for sync");
        assert_eq!(for_codegen["cached"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            for_codegen["children"][0]["children"][0]["codegenNode"]["children"]["arguments"][1]
                ["returns"]["props"]["content"],
            json!("_hoisted_1")
        );
        assert_eq!(
            for_codegen["children"][0]["children"][0]["codegenNode"]["children"]["arguments"][1]
                ["returns"]["children"]["type"],
            json!(20)
        );
    }

    #[test]
    fn vue3_transform_bind_suite_materializes_public_vnode_props() {
        let transformed = dispatch(
            "vue3.core.transformBindSuite",
            json!({ "source": r#"<div v-bind:id="id"/>"#, "options": {} }),
        )
        .expect("transformBind suite");
        let props = &transformed["codegenNode"]["props"];
        assert_eq!(props["type"], json!(15));
        assert_eq!(props["properties"][0]["key"]["content"], json!("id"));
        assert_eq!(props["properties"][0]["key"]["isStatic"], json!(true));
        assert_eq!(props["properties"][0]["value"]["content"], json!("id"));
        assert_eq!(props["properties"][0]["value"]["isStatic"], json!(false));

        let shorthand = dispatch(
            "vue3.core.transformBindSuite",
            json!({ "source": "<div :id />", "options": {} }),
        )
        .expect("transformBind suite shorthand");
        let shorthand_props = &shorthand["codegenNode"]["props"];
        assert_eq!(
            shorthand_props["properties"][0]["value"]["content"],
            json!("id")
        );
        assert_eq!(
            shorthand_props["properties"][0]["value"]["isStatic"],
            json!(false)
        );

        let dynamic = dispatch(
            "vue3.core.transformBindSuite",
            json!({
                "source": r#"<div v-bind:[foo(bar)].camel="id"/>"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformBind suite dynamic arg");
        let dynamic_props = &dynamic["codegenNode"]["props"];
        assert_eq!(dynamic_props["type"], json!(14));
        assert_eq!(dynamic_props["callee"], json!("NORMALIZE_PROPS"));
        let key = &dynamic_props["arguments"][0]["properties"][0]["key"];
        assert_eq!(key["type"], json!(8));
        assert_eq!(key["children"][0], json!("_camelize("));
        assert_eq!(key["children"][2]["content"], json!("_ctx.foo"));
        assert_eq!(key["children"][4]["content"], json!("_ctx.bar"));
        assert_eq!(
            dynamic_props["arguments"][0]["properties"][0]["value"]["content"],
            json!("_ctx.id")
        );

        let invalid = dispatch(
            "vue3.core.transformBindSuite",
            json!({ "source": "<div v-bind:[arg] />", "options": {} }),
        )
        .expect("transformBind suite invalid shorthand");
        assert_eq!(invalid["__vuecErrors"][0]["code"], json!(53));
        assert_eq!(invalid["__vuecErrors"][0]["loc"]["source"], json!("[arg]"));
    }

    #[test]
    fn vue3_transform_for_suite_materializes_ref_for_props() {
        let static_ref = dispatch(
            "vue3.core.transformForSuite",
            json!({ "source": r#"<div v-for="i in list" ref="x"/>"#, "options": {} }),
        )
        .expect("transformFor suite static ref");
        let props = &static_ref["node"]["children"][0]["codegenNode"]["props"]["properties"];
        assert_eq!(props[0]["key"]["content"], json!("ref_for"));
        assert_eq!(props[0]["value"]["content"], json!("true"));
        assert_eq!(props[0]["value"]["isStatic"], json!(false));
        assert_eq!(props[1]["key"]["content"], json!("ref"));
        assert_eq!(props[1]["value"]["content"], json!("x"));

        let dynamic_ref = dispatch(
            "vue3.core.transformForSuite",
            json!({ "source": r#"<div v-for="i in list" :ref="x"/>"#, "options": {} }),
        )
        .expect("transformFor suite dynamic ref");
        let props = &dynamic_ref["node"]["children"][0]["codegenNode"]["props"]["properties"];
        assert_eq!(props[0]["key"]["content"], json!("ref_for"));
        assert_eq!(props[1]["key"]["content"], json!("ref"));
        assert_eq!(props[1]["value"]["content"], json!("x"));
        assert_eq!(props[1]["value"]["isStatic"], json!(false));

        let object_bind = dispatch(
            "vue3.core.transformForSuite",
            json!({ "source": r#"<div v-for="i in list" v-bind="x"/>"#, "options": {} }),
        )
        .expect("transformFor suite object bind");
        let props = &object_bind["node"]["children"][0]["codegenNode"]["props"];
        assert_eq!(props["type"], json!(14));
        assert_eq!(props["callee"], json!("MERGE_PROPS"));
        assert_eq!(
            props["arguments"][0]["properties"][0]["key"]["content"],
            json!("ref_for")
        );
        assert_eq!(props["arguments"][1]["content"], json!("x"));
        assert_eq!(props["arguments"][1]["isStatic"], json!(false));
    }

    #[test]
    fn vue3_transform_if_suite_materializes_branch_chain_and_keys() {
        let transformed = dispatch(
            "vue3.core.transformIfSuite",
            json!({
                "source": r#"<div v-if="ok" /><p v-else-if="next" /><span v-else />"#,
                "options": {},
            }),
        )
        .expect("transformIf suite branch chain");

        let branches = transformed["node"]["branches"]
            .as_array()
            .expect("if branches");
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0]["condition"]["content"], json!("ok"));
        assert_eq!(branches[1]["condition"]["content"], json!("next"));
        assert!(branches[2]["condition"].is_null());

        let first_codegen = &branches[0]["children"][0]["codegenNode"];
        assert_eq!(first_codegen["type"], json!(13));
        assert_eq!(first_codegen["isBlock"], json!(true));
        assert_eq!(
            first_codegen["props"]["properties"][0]["key"]["content"],
            json!("key")
        );
        assert_eq!(
            first_codegen["props"]["properties"][0]["value"]["content"],
            json!("0")
        );

        let second_codegen = &transformed["node"]["codegenNode"]["alternate"]["consequent"];
        assert_eq!(second_codegen["tag"], json!("\"p\""));
        assert_eq!(
            second_codegen["props"]["properties"][0]["value"]["content"],
            json!("1")
        );
        let third_codegen = &transformed["node"]["codegenNode"]["alternate"]["alternate"];
        assert_eq!(third_codegen["tag"], json!("\"span\""));
        assert_eq!(
            third_codegen["props"]["properties"][0]["value"]["content"],
            json!("2")
        );
        assert_eq!(
            transformed["root"]["helpers"],
            json!(["OPEN_BLOCK", "CREATE_ELEMENT_BLOCK", "CREATE_COMMENT"])
        );
    }

    #[test]
    fn vue3_transform_if_suite_materializes_slot_and_object_on_key_injection() {
        let slot = dispatch(
            "vue3.core.transformIfSuite",
            json!({ "source": r#"<template v-if="ok"><slot/></template>"#, "options": {} }),
        )
        .expect("transformIf suite slot outlet branch");
        let slot_call = &slot["node"]["codegenNode"]["consequent"];
        assert_eq!(slot_call["callee"], json!("RENDER_SLOT"));
        assert_eq!(slot_call["arguments"][0], json!("$slots"));
        assert_eq!(slot_call["arguments"][1], json!("\"default\""));
        assert_eq!(
            slot_call["arguments"][2]["properties"][0]["key"]["content"],
            json!("key")
        );
        assert_eq!(
            slot_call["arguments"][2]["properties"][0]["value"]["content"],
            json!("0")
        );
        assert_eq!(
            slot["root"]["helpers"],
            json!(["RENDER_SLOT", "CREATE_COMMENT"])
        );

        let object_on = dispatch(
            "vue3.core.transformIfSuite",
            json!({ "source": r#"<div v-if="ok" v-on="handlers" />"#, "options": {} }),
        )
        .expect("transformIf suite object v-on branch");
        let props = &object_on["node"]["codegenNode"]["consequent"]["props"];
        assert_eq!(props["type"], json!(14));
        assert_eq!(props["callee"], json!("MERGE_PROPS"));
        assert_eq!(
            props["arguments"][0]["properties"][0]["key"]["content"],
            json!("key")
        );
        assert_eq!(
            props["arguments"][0]["properties"][0]["value"]["content"],
            json!("0")
        );
        assert_eq!(props["arguments"][1]["callee"], json!("TO_HANDLERS"));
        assert_eq!(
            props["arguments"][1]["arguments"][0]["content"],
            json!("handlers")
        );
        assert_eq!(props["arguments"][1]["arguments"][1], json!("true"));
    }

    #[test]
    fn vue3_transform_if_suite_reports_public_errors() {
        let missing_expression = dispatch(
            "vue3.core.transformIfSuite",
            json!({ "source": "<div v-if />", "options": {} }),
        )
        .expect("transformIf suite missing expression");
        assert_eq!(
            missing_expression["root"]["__vuecErrors"][0]["code"],
            json!(28)
        );

        let duplicate_key = dispatch(
            "vue3.core.transformIfSuite",
            json!({
                "source": r#"<div v-if="ok" key="same" /><p v-else key="same" />"#,
                "options": {},
            }),
        )
        .expect("transformIf suite duplicate key");
        assert_eq!(duplicate_key["root"]["__vuecErrors"][0]["code"], json!(29));

        let adjacent_else_if = dispatch(
            "vue3.core.transformIfSuite",
            json!({
                "source": r#"<div v-if="ok" /><p v-else /><span v-else-if="late" />"#,
                "options": {},
            }),
        )
        .expect("transformIf suite adjacent else-if");
        assert!(adjacent_else_if["root"]["__vuecErrors"]
            .as_array()
            .expect("if errors")
            .iter()
            .any(|error| error["code"] == json!(30)));
    }

    #[test]
    fn vue3_transform_expression_suite_processes_public_ast_expressions() {
        let transformed = dispatch(
            "vue3.core.transformExpressionSuite",
            json!({ "source": r#"<div v-foo:[arg]="baz">{{ foo }}</div>"#, "options": {} }),
        )
        .expect("transformExpression suite");

        assert_eq!(transformed["props"][0]["arg"]["content"], json!("_ctx.arg"));
        assert_eq!(transformed["props"][0]["exp"]["content"], json!("_ctx.baz"));
        assert_eq!(
            transformed["children"][0]["content"]["content"],
            json!("_ctx.foo")
        );
        assert_eq!(transformed["__vuecErrors"], json!([]));
    }

    #[test]
    fn vue3_transform_expression_suite_reports_expression_errors() {
        let transformed = dispatch(
            "vue3.core.transformExpressionSuite",
            json!({ "source": "{{ a( }}", "options": {} }),
        )
        .expect("transformExpression suite parse error");

        assert_eq!(transformed["__vuecErrors"][0]["code"], json!(46));
        assert_eq!(
            transformed["__vuecErrors"][0]["message"],
            json!("Error parsing JavaScript expression: Unexpected token")
        );
    }

    #[test]
    fn vue3_transform_expression_suite_preserves_plugin_and_binding_options() {
        let pipeline = dispatch(
            "vue3.core.transformExpressionSuite",
            json!({
                "source": "{{ a |> uppercase }}",
                "options": {
                    "expressionPlugins": [["pipelineOperator", { "proposal": "minimal" }]]
                }
            }),
        )
        .expect("transformExpression suite pipeline");
        assert_eq!(
            pipeline["content"]["children"][0]["content"],
            json!("_ctx.a")
        );
        assert_eq!(
            pipeline["content"]["children"][2]["content"],
            json!("_ctx.uppercase")
        );

        let inline_assignment = dispatch(
            "vue3.core.transformExpressionSuite",
            json!({
                "source": "{{ (async () => { x = await bar })() }}",
                "options": {
                    "inline": true,
                    "bindingMetadata": {
                        "x": "setup-let",
                        "bar": "setup-const"
                    }
                }
            }),
        )
        .expect("transformExpression suite binding metadata");
        assert_eq!(
            inline_assignment["content"]["children"][1]["content"],
            json!("_isRef(x) ? x.value = await bar : x")
        );
        assert_eq!(
            inline_assignment["content"]["children"][3]["content"],
            json!("bar")
        );
    }

    #[test]
    fn vue3_transform_model_suite_materializes_public_vnode_props() {
        let transformed = dispatch(
            "vue3.core.transformModelSuite",
            json!({ "source": r#"<input v-model="model" />"#, "options": {} }),
        )
        .expect("transformModel suite");
        let props = &transformed["children"][0]["codegenNode"]["props"]["properties"];
        assert_eq!(props[0]["key"]["content"], json!("modelValue"));
        assert_eq!(props[0]["value"]["content"], json!("model"));
        assert_eq!(props[1]["key"]["content"], json!("onUpdate:modelValue"));
        assert_eq!(props[1]["value"]["children"][0], json!("$event => (("));
        assert_eq!(props[1]["value"]["children"][1]["content"], json!("model"));
        assert_eq!(props[1]["value"]["children"][2], json!(") = $event)"));
        assert_eq!(
            transformed["children"][0]["codegenNode"]["dynamicProps"],
            json!("[\"modelValue\", \"onUpdate:modelValue\"]")
        );

        let prefixed = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<input v-model="model[index]" />"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformModel suite prefixed");
        let value_children =
            &prefixed["children"][0]["codegenNode"]["props"]["properties"][0]["value"]["children"];
        assert_eq!(value_children[0]["content"], json!("_ctx.model"));
        assert_eq!(value_children[2]["content"], json!("_ctx.index"));

        let dynamic = dispatch(
            "vue3.core.transformModelSuite",
            json!({ "source": r#"<input v-model:[value]="model" />"#, "options": {} }),
        )
        .expect("transformModel suite dynamic arg");
        let dynamic_props = &dynamic["children"][0]["codegenNode"]["props"];
        assert_eq!(dynamic_props["type"], json!(14));
        assert_eq!(dynamic_props["callee"], json!("NORMALIZE_PROPS"));
        assert_eq!(
            dynamic_props["arguments"][0]["properties"][1]["key"]["children"][0],
            json!("\"onUpdate:\" + ")
        );

        let cached = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<input v-model="foo" />"#,
                "options": { "prefixIdentifiers": true, "cacheHandlers": true },
            }),
        )
        .expect("transformModel suite cached handler");
        assert_eq!(cached["cached"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            cached["children"][0]["codegenNode"]["dynamicProps"],
            json!("[\"modelValue\"]")
        );
        assert_eq!(
            cached["children"][0]["codegenNode"]["props"]["properties"][1]["value"]["type"],
            json!(20)
        );
    }

    #[test]
    fn vue3_transform_model_suite_tracks_scopes_modifiers_and_errors() {
        let scoped = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<input v-for="i in list" v-model="foo[i]" />"#,
                "options": { "prefixIdentifiers": true, "cacheHandlers": true },
            }),
        )
        .expect("transformModel suite v-for");
        assert_eq!(scoped["children"][0]["type"], json!(11));
        assert_eq!(scoped["cached"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            scoped["children"][0]["children"][0]["codegenNode"]["dynamicProps"],
            json!("[\"modelValue\", \"onUpdate:modelValue\"]")
        );
        assert_ne!(
            scoped["children"][0]["children"][0]["codegenNode"]["props"]["properties"][1]["value"]
                ["type"],
            json!(20)
        );

        let once = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<div v-once><input v-model="foo" /></div>"#,
                "options": { "prefixIdentifiers": true, "cacheHandlers": true },
            }),
        )
        .expect("transformModel suite v-once");
        assert_eq!(once["cached"].as_array().map(Vec::len), Some(1));

        let slot = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<Comp v-slot="{ foo }"><input v-model="foo.bar"/></Comp>"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformModel suite slot scope");
        assert_eq!(
            slot["children"][0]["children"][0]["codegenNode"]["dynamicProps"],
            json!("[\"modelValue\", \"onUpdate:modelValue\"]")
        );

        let modifiers = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<Comp v-model:foo.trim="foo" v-model:bar.number="bar" />"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformModel suite modifiers");
        let properties = &modifiers["children"][0]["codegenNode"]["props"]["properties"];
        assert_eq!(properties[2]["key"]["content"], json!("fooModifiers"));
        assert_eq!(properties[2]["value"]["content"], json!("{ trim: true }"));
        assert_eq!(properties[5]["key"]["content"], json!("barModifiers"));
        assert_eq!(
            modifiers["children"][0]["codegenNode"]["dynamicProps"],
            json!("[\"foo\", \"onUpdate:foo\", \"bar\", \"onUpdate:bar\"]")
        );

        let missing = dispatch(
            "vue3.core.transformModelSuite",
            json!({ "source": "<span v-model />", "options": {} }),
        )
        .expect("transformModel suite missing expression");
        assert_eq!(missing["__vuecErrors"][0]["code"], json!(41));
        let malformed = dispatch(
            "vue3.core.transformModelSuite",
            json!({ "source": r#"<span v-model="a + b" />"#, "options": {} }),
        )
        .expect("transformModel suite malformed expression");
        assert_eq!(malformed["__vuecErrors"][0]["code"], json!(42));
        let prop = dispatch(
            "vue3.core.transformModelSuite",
            json!({
                "source": r#"<div v-model="p" />"#,
                "options": { "bindingMetadata": { "p": "props" } },
            }),
        )
        .expect("transformModel suite props binding");
        assert_eq!(prop["__vuecErrors"][0]["code"], json!(44));
    }

    #[test]
    fn vue3_transform_on_suite_materializes_event_handlers() {
        let transformed = dispatch(
            "vue3.core.transformOnSuite",
            json!({ "source": r#"<div v-on:click="onClick"/>"#, "options": {} }),
        )
        .expect("transformOn suite");
        let props = &transformed["node"]["codegenNode"]["props"]["properties"];
        assert_eq!(props[0]["key"]["content"], json!("onClick"));
        assert_eq!(props[0]["key"]["isStatic"], json!(true));
        assert_eq!(props[0]["key"]["loc"]["source"], json!("click"));
        assert_eq!(props[0]["value"]["content"], json!("onClick"));
        assert_eq!(props[0]["value"]["isStatic"], json!(false));
        assert_eq!(props[0]["value"]["loc"]["source"], json!("onClick"));

        let dynamic = dispatch(
            "vue3.core.transformOnSuite",
            json!({
                "source": r#"<div v-on:[event(foo)]="handler"/>"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformOn suite dynamic arg");
        let key = &dynamic["node"]["codegenNode"]["props"]["properties"][0]["key"]["children"];
        assert_eq!(key[0], json!("_toHandlerKey("));
        assert_eq!(key[1]["content"], json!("_ctx.event"));
        assert_eq!(key[3]["content"], json!("_ctx.foo"));
        assert_eq!(
            dynamic["node"]["codegenNode"]["props"]["properties"][0]["value"]["content"],
            json!("_ctx.handler")
        );
        assert!(dynamic["root"]["helpers"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|helper| helper == "TO_HANDLER_KEY"));

        let inline = dispatch(
            "vue3.core.transformOnSuite",
            json!({ "source": r#"<div @click="i++"/>"#, "options": {} }),
        )
        .expect("transformOn suite inline");
        let value = &inline["node"]["codegenNode"]["props"]["properties"][0]["value"];
        assert_eq!(value["children"][0], json!("$event => ("));
        assert_eq!(value["children"][1]["content"], json!("i++"));
        assert_eq!(value["children"][2], json!(")"));

        let prefixed_member = dispatch(
            "vue3.core.transformOnSuite",
            json!({
                "source": r#"<div @click="a['b' + c]"/>"#,
                "options": { "prefixIdentifiers": true },
            }),
        )
        .expect("transformOn suite prefixed member");
        let member =
            &prefixed_member["node"]["codegenNode"]["props"]["properties"][0]["value"]["children"];
        assert_eq!(member[0]["content"], json!("_ctx.a"));
        assert_eq!(member[1], json!("['b' + "));
        assert_eq!(member[2]["content"], json!("_ctx.c"));
        assert_eq!(member[3], json!("]"));
    }

    #[test]
    fn vue3_transform_on_suite_tracks_cache_scopes_and_errors() {
        let cached = dispatch(
            "vue3.core.transformOnSuite",
            json!({
                "source": r#"<div v-on:click="foo" />"#,
                "options": { "prefixIdentifiers": true, "cacheHandlers": true },
            }),
        )
        .expect("transformOn suite cached member");
        assert_eq!(cached["root"]["cached"].as_array().map(Vec::len), Some(1));
        let value = &cached["node"]["codegenNode"]["props"]["properties"][0]["value"];
        assert_eq!(value["type"], json!(20));
        assert_eq!(value["index"], json!(0));
        assert_eq!(value["value"]["children"][0], json!("(...args) => ("));
        assert_eq!(
            value["value"]["children"][1]["content"],
            json!("_ctx.foo && _ctx.foo(...args)")
        );
        assert_eq!(cached["node"]["codegenNode"]["patchFlag"], Value::Null);
        assert_eq!(cached["node"]["codegenNode"]["dynamicProps"], Value::Null);

        let component = dispatch(
            "vue3.core.transformOnSuite",
            json!({
                "source": r#"<comp v-on:click="foo" />"#,
                "options": {
                    "prefixIdentifiers": true,
                    "cacheHandlers": true,
                    "__vuecNativeTags": ["div"]
                },
            }),
        )
        .expect("transformOn suite component member");
        assert_eq!(
            component["root"]["cached"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            component["node"]["codegenNode"]["props"]["properties"][0]["value"]["content"],
            json!("_ctx.foo")
        );

        let once = dispatch(
            "vue3.core.transformOnSuite",
            json!({
                "source": r#"<div v-once><div v-on:click="foo"/></div>"#,
                "options": { "prefixIdentifiers": true, "cacheHandlers": true },
            }),
        )
        .expect("transformOn suite v-once");
        assert_eq!(once["root"]["cached"].as_array().map(Vec::len), Some(1));

        let scoped = dispatch(
            "vue3.core.transformOnSuite",
            json!({
                "source": r#"<div v-for="项 in items" :key="value"><div v-on:click="foo(项)"/></div>"#,
                "options": { "prefixIdentifiers": true, "cacheHandlers": true },
            }),
        )
        .expect("transformOn suite unicode v-for scope");
        assert_eq!(scoped["root"]["cached"].as_array().map(Vec::len), Some(0));

        let missing = dispatch(
            "vue3.core.transformOnSuite",
            json!({ "source": "<div v-on:click />", "options": {} }),
        )
        .expect("transformOn suite missing expression");
        assert_eq!(missing["root"]["__vuecErrors"][0]["code"], json!(35));
        assert_eq!(
            missing["root"]["__vuecErrors"][0]["loc"]["source"],
            json!("v-on:click")
        );

        let vnode_hook = dispatch(
            "vue3.core.transformOnSuite",
            json!({ "source": r#"<div v-on:vnode-mounted="onMount"/>"#, "options": {} }),
        )
        .expect("transformOn suite vnode hook error");
        assert_eq!(vnode_hook["root"]["__vuecErrors"][0]["code"], json!(52));
        assert_eq!(
            vnode_hook["root"]["__vuecErrors"][0]["loc"]["source"],
            json!("vnode-mounted")
        );
    }
