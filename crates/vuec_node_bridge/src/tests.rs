#[cfg(test)]
mod tests {
    use crate::*;
    use std::collections::BTreeSet;
    use vuec_bridge_registry::bridge_commands;

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
        let dispatch = include_str!("dispatch.rs");
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

    #[test]
    fn vue27_bridge_compile_style_rewrites_css_vars_with_default_scope() {
        let compiled = dispatch(
            "sfc.vue27.compileStyle",
            json!({
                "source": ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
                "filename": "test.css",
                "options": {
                    "id": "data-v-test"
                }
            }),
        )
        .expect("vue27 style");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(".foo[data-v-test]"));
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_style_compiles_raw_css_source() {
        let compiled = dispatch(
            "sfc.compileStyle",
            json!({
                "source": ".foo { color: red; }",
                "filename": "test.css",
                "options": {
                    "id": "data-v-test",
                    "scoped": true
                }
            }),
        )
        .expect("vue3 style");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(".foo[data-v-test] { color: red;"));
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert_eq!(compiled["rawResult"], json!(["postcss-result"]));

        let modules = dispatch(
            "sfc.compileStyleAsync",
            json!({
                "source": ".red { color: red; } :global(.blue) { color: blue; }",
                "filename": "test.css",
                "options": {
                    "id": "test",
                    "modules": true
                }
            }),
        )
        .expect("vue3 style modules");

        assert!(modules["modules"]["red"]
            .as_str()
            .unwrap_or("")
            .contains("_red_"));
        assert!(modules["modules"].get("blue").is_none());
    }

    #[test]
    fn vue27_bridge_parse_collects_comment_separated_css_vars() {
        let parsed = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": r#"<style>.foo { color: v-bind/**/(color); font-size: v-bind /*x*/ ('font.size'); }</style>"#,
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse");

        assert_eq!(parsed["cssVars"], json!(["color", "font.size"]));
    }

    #[test]
    fn vue27_bridge_parse_uses_legacy_deindent() {
        let parsed = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": "<template>\n  <div id=\"app\">\n    <router-view />\n  </div>\n</template>",
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse");

        assert_eq!(
            parsed["template"]["content"],
            json!("\n<div id=\"app\">\n  <router-view />\n</div>\n")
        );
    }

    #[test]
    fn vue27_bridge_parse_projects_errors_by_source_range_option() {
        let source = r#"<template>
<div>
  <input>
</div>
</template>"#;
        let default = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": source,
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse default errors");
        assert_eq!(
            default["errors"],
            json!(["tag <input> has no matching end tag."])
        );

        let ranged = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": source,
                "filename": "test.vue",
                "options": {
                    "outputSourceRange": true
                }
            }),
        )
        .expect("vue27 parse ranged errors");
        let errors = ranged["errors"].as_array().expect("ranged errors");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0]["msg"],
            json!("tag <input> has no matching end tag.")
        );
        assert!(errors[0]["start"].as_u64().is_some());
        assert!(errors[0]["end"].as_u64().is_some());
    }

    #[test]
    fn vue3_sfc_bridge_rewrite_default_routes_parser_plugins() {
        let rewritten = dispatch(
            "sfc.rewriteDefault",
            json!({
                "source": "export { foo as default, bar } from './index.js'",
                "variable": "script",
                "plugins": []
            }),
        )
        .expect("vue3 rewriteDefault");
        assert_eq!(
            rewritten,
            json!("import { foo as __VUE_DEFAULT__ } from './index.js'\nexport {  bar } from './index.js'\nconst script = __VUE_DEFAULT__")
        );

        let without_ts = dispatch(
            "sfc.rewriteDefault",
            json!({
                "source": "export default interface Foo {}",
                "variable": "__default__",
                "plugins": []
            }),
        )
        .unwrap_err();
        assert!(format!("{without_ts:#}").contains("Unexpected reserved word 'interface'. (1:15)"));

        let with_ts = dispatch(
            "sfc.rewriteDefault",
            json!({
                "source": "export default interface Foo {}",
                "variable": "__default__",
                "plugins": [["typescript", {}]]
            }),
        )
        .expect("vue3 TypeScript rewriteDefault");
        assert_eq!(with_ts, json!("const __default__ = interface Foo {}"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_normal_default_export_with_setup() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("const __default__ = { name: 'X' }"));
        assert!(content.contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(content.contains("const a = 1\nconst __returned__ = { a }"));

        let script_ast = compiled["scriptAst"].as_array().expect("scriptAst array");
        assert_eq!(script_ast.len(), 1);
        assert_eq!(script_ast[0]["type"], json!("ExportDefaultDeclaration"));
        assert_eq!(
            script_ast[0]["source"],
            json!("export default { name: 'X' }")
        );
        assert_eq!(
            script_ast[0]["declaration"]["type"],
            json!("ObjectExpression")
        );
        assert_eq!(script_ast[0]["loc"]["start"]["offset"], json!(0));

        let setup_ast = compiled["scriptSetupAst"]
            .as_array()
            .expect("scriptSetupAst array");
        assert_eq!(setup_ast.len(), 1);
        assert_eq!(setup_ast[0]["type"], json!("VariableDeclaration"));
        assert_eq!(setup_ast[0]["kind"], json!("const"));
        assert_eq!(setup_ast[0]["source"], json!("const a = 1"));
        assert_eq!(setup_ast[0]["declarations"][0]["id"]["name"], json!("a"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_internal_script_ast_mode() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
                "filename": "Comp.vue",
                "options": {
                    "__vuecScriptAstMode": "none"
                }
            }),
        )
        .expect("vue3 compileScript");

        assert!(compiled.get("scriptAst").is_none());
        assert!(compiled.get("scriptSetupAst").is_none());

        let top_level = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script>",
                "filename": "Comp.vue",
                "options": {
                    "scriptAstMode": "top-level"
                }
            }),
        )
        .expect("vue3 compileScript top-level AST");
        let script_ast = top_level["scriptAst"].as_array().expect("scriptAst array");
        assert_eq!(script_ast[0]["type"], json!("ExportDefaultDeclaration"));
        assert!(script_ast[0].get("declaration").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_gen_default_as_option() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>const a = 1</script>",
                "filename": "Comp.vue",
                "options": {
                    "genDefaultAs": "_sfc_"
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("const _sfc_ = {"));
        assert!(!content.contains("export default"));

        let snake_case = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script>export default { name: 'X' }</script>",
                "filename": "Comp.vue",
                "options": {
                    "gen_default_as": "_sfc_"
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = snake_case["content"].as_str().unwrap_or_default();
        assert!(content.contains("const _sfc_ = { name: 'X' }"));
        assert!(!content.contains("export default"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_import_attributes_parser_option() {
        let with_syntax = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>import { foo } from './foo.js' with { type: 'json' }</script>",
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(with_syntax["errors"].as_array().unwrap().is_empty());

        let assert_syntax = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>import { foo } from './foo.js' assert { type: 'json' }</script>",
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(assert_syntax["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error
                    .as_str()
                    .is_some_and(|error| error.contains("import attributes is deprecated"))
            }));

        let overridden = dispatch(
            "sfc.compileScript",
            json!({
                "source": "<script setup>import { foo } from './foo.js' assert { type: 'json' }</script>",
                "filename": "Comp.vue",
                "options": {
                    "babelParserPlugins": [
                        ["importAttributes", { "deprecatedAssertSyntax": true }]
                    ]
                }
            }),
        )
        .expect("vue3 compileScript");
        assert!(overridden["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_projects_source_map_option() {
        let source = "<script setup>\nconst count = 1\n</script>";
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": source,
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        assert_eq!(compiled["map"]["version"], json!(3));
        assert_eq!(compiled["map"]["sources"], json!(["FooBar.vue"]));
        assert_eq!(compiled["map"]["sourcesContent"][0], json!(source));
        assert!(compiled["map"]["mappings"]
            .as_str()
            .is_some_and(|mappings| !mappings.is_empty()));

        let disabled = dispatch(
            "sfc.compileScript",
            json!({
                "source": source,
                "filename": "FooBar.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 compileScript");
        assert!(disabled["map"].is_null());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_normal_script_bindings() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script>",
                    "const ignored = 1\n",
                    "export default {",
                    "props: ['foo'],",
                    "inject: { service: {} },",
                    "data() { return { count: 1 } },",
                    "methods: { save() {} }",
                    "}",
                    "</script>"
                ),
                "filename": "Comp.vue"
            }),
        )
        .expect("vue3 compileScript");

        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["service"], json!("options"));
        assert_eq!(compiled["bindings"]["count"], json!("data"));
        assert_eq!(compiled["bindings"]["save"], json!("options"));
        assert_eq!(compiled["bindings"]["__isScriptSetup"], json!("false"));
        assert!(compiled["bindings"].get("ignored").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_generates_runtime_macros() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const props = defineProps({ foo: String })\n",
                    "const emit = defineEmits(['save'])\n",
                    "defineExpose({ reset() {} })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("props: { foo: String },"));
        assert!(content.contains("emits: ['save'],"));
        assert!(content.contains("setup(__props, { expose: __expose, emit: __emit })"));
        assert!(content.contains("const props = __props"));
        assert!(content.contains("const emit = __emit"));
        assert!(content.contains("__expose({ reset() {} })"));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["props"], json!("setup-reactive-const"));
        assert_eq!(compiled["bindings"]["emit"], json!("setup-const"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_rewrites_define_slots() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { defineSlots, ref } from 'vue'\n",
                    "const slots = defineSlots<{ default: { msg: string } }>()\n",
                    "const count = ref(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(
            "import { useSlots as _useSlots, defineComponent as _defineComponent } from 'vue'"
        ));
        assert!(content.contains("import { ref } from 'vue'"));
        assert!(content.contains("const slots = _useSlots()"));
        assert!(content.contains("const __returned__ = { slots, count, ref }"));
        assert!(!content.contains("defineSlots"));
        assert_eq!(compiled["bindings"]["slots"], json!("setup-const"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert!(compiled["bindings"].get("defineSlots").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_duplicate_define_expose() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "defineExpose({ first: true })\n",
                    "defineExpose({ second: true })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().iter().any(|error| {
            error
                .as_str()
                .is_some_and(|error| error.contains("duplicate defineExpose() call"))
        }));
        assert!(content.contains("__expose({ first: true })"));
        assert!(content.contains("__expose({ second: true })"));
        assert!(!content.contains("defineExpose"));
        assert!(!content.contains("__expose();"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_infers_typescript_macros() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "type Props = { foo?: string; ok?: boolean; cb?: () => void }\n",
                    "const props = withDefaults(defineProps<Props>(), { foo: 'x', ok: true })\n",
                    "const emit = defineEmits<{(e: 'save'): void}>()",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "isProd": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("foo: { default: 'x' }"));
        assert!(content.contains("ok: { type: Boolean, default: true }"));
        assert!(content.contains("cb: {}"));
        assert!(content.contains(r#"emits: ["save"],"#));
        assert!(content.contains("setup(__props: any, { expose: __expose, emit: __emit })"));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["ok"], json!("props"));
        assert_eq!(compiled["bindings"]["cb"], json!("props"));
        assert_eq!(compiled["bindings"]["props"], json!("setup-const"));
        assert_eq!(compiled["bindings"]["emit"], json!("setup-const"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_passes_custom_element_prod_option() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "withDefaults(defineProps<{ foo?: number; bar?: string }>(), { foo: 5.5 })",
                    "</script>"
                ),
                "filename": "Foo.ce.vue",
                "options": {
                    "isProd": true,
                    "customElement": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(content.contains("foo: { default: 5.5, type: Number }"));
        assert!(content.contains("bar: {type: String}"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_with_defaults_errors() {
        let bad_first = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const props = withDefaults(foo(), { foo: 'x' })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(bad_first["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error
                    .contains("withDefaults' first argument must be a defineProps call"))));
        assert!(!bad_first["content"]
            .as_str()
            .unwrap_or_default()
            .contains("withDefaults"));

        let runtime_props = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const props = withDefaults(defineProps({ foo: String }), { foo: 'x' })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let runtime_content = runtime_props["content"].as_str().unwrap_or_default();
        assert!(runtime_props["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "withDefaults can only be used with type-based defineProps declaration"
            ))));
        assert!(runtime_content.contains("props: { foo: String },"));
        assert!(!runtime_content.contains("withDefaults"));
        assert!(!runtime_content.contains("defineProps"));

        let destructure = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo } = withDefaults(defineProps<{ foo: string }>(), { foo: 'foo' })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let destructure_content = destructure["content"].as_str().unwrap_or_default();
        assert!(destructure["errors"].as_array().unwrap().is_empty());
        assert!(destructure["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .is_some_and(|warning| warning
                    .contains("withDefaults() is unnecessary when using destructure"))));
        assert!(destructure_content.contains("const { foo } = __props"));
        assert_eq!(destructure["bindings"]["foo"], json!("setup-const"));

        let missing_defaults = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const props = withDefaults(defineProps<{ foo?: string }>())",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(missing_defaults["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(
                |error| error.contains("The 2nd argument of withDefaults is required")
            )));
        assert!(!missing_defaults["content"]
            .as_str()
            .unwrap_or_default()
            .contains("withDefaults"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_duplicate_props_and_emits() {
        let duplicate_props = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<{ foo?: string }>()\n",
                    "const props = withDefaults(defineProps<{ bar?: number }>(), { bar: 1 })",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let props_content = duplicate_props["content"].as_str().unwrap_or_default();
        assert!(duplicate_props["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("duplicate defineProps() call"))));
        assert!(!props_content.contains("defineProps"));
        assert!(!props_content.contains("withDefaults"));

        let duplicate_emits = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "defineEmits(['save'])\n",
                    "const emit = defineEmits(['cancel'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let emits_content = duplicate_emits["content"].as_str().unwrap_or_default();
        assert!(duplicate_emits["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("duplicate defineEmits() call"))));
        assert!(emits_content.contains("const emit = __emit"));
        assert!(!emits_content.contains("defineEmits"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_type_resolution_errors() {
        let unresolved = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<X>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(unresolved["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error.as_str().is_some_and(|error| {
                    error.contains(
                        "Unresolvable type reference or unsupported built-in utility type",
                    )
                })
            }));

        let missing_import = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { X } from './foo'\n",
                    "defineProps<X>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(missing_import["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error.as_str().is_some_and(|error| {
                    error.contains("Failed to resolve import source \"./foo\".")
                })
            }));

        let silent_member = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type P from 'unknown'\n",
                    "defineProps<{ foo: T, bar: T['bar'], baz: P }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(silent_member["errors"].as_array().unwrap().is_empty());
        assert_eq!(silent_member["bindings"]["foo"], json!("props"));
        assert_eq!(silent_member["bindings"]["bar"], json!("props"));
        assert_eq!(silent_member["bindings"]["baz"], json!("props"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_destructure_errors() {
        let dynamic_key = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const key = 'foo'\n",
                    "const { [key]: foo } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(dynamic_key["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("destructure cannot use computed key"))));

        let nested_pattern = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo: { bar } } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(nested_pattern["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(
                |error| error.contains("destructure does not support nested patterns")
            )));

        let local_default = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "let x = 1\n",
                    "const { foo = () => x } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(local_default["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(
                |error| error.contains("cannot reference locally declared variables")
            )));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_props_destructure_option() {
        let disabled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo, bar: baz } = defineProps(['foo', 'bar'])\n",
                    "const message = foo + baz",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "propsDestructure": false
                }
            }),
        )
        .expect("vue3 compileScript");
        let content = disabled["content"].as_str().unwrap_or_default();
        assert!(disabled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("const { foo, bar: baz } = __props"));
        assert!(content.contains("const message = foo + baz"));
        assert!(!content.contains("__props.foo + __props.bar"));
        assert_eq!(disabled["bindings"]["foo"], json!("setup-const"));
        assert_eq!(disabled["bindings"]["baz"], json!("setup-const"));

        let errored = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo } = defineProps(['foo'])",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "propsDestructure": "error"
                }
            }),
        )
        .expect("vue3 compileScript");
        assert!(errored["errors"].as_array().unwrap().iter().any(|error| {
            error.as_str().is_some_and(|error| {
                error.contains("Props destructure is explicitly prohibited via config.")
            })
        }));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_destructure_usage_errors() {
        let assignment = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo } = defineProps(['foo'])\n",
                    "foo = 'bar'",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(assignment["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("Cannot assign to destructured props"))));

        let watch_alias = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { watch as w, toRef as r } from 'vue'\n",
                    "const { foo, bar } = defineProps(['foo', 'bar'])\n",
                    "w(foo, () => {})\n",
                    "r(bar)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let errors = watch_alias["errors"].as_array().unwrap();
        assert!(errors
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch()."
            ))));
        assert!(errors
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "\"bar\" is a destructured prop and should not be passed directly to toRef()."
            ))));

        let normal_script_watch_alias = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script>",
                    "import { watch as w } from 'vue'",
                    "</script>",
                    "<script setup>",
                    "const { foo } = defineProps(['foo'])\n",
                    "w(foo, () => {})",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(normal_script_watch_alias["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch()."
            ))));

        let shadowed = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { watch } from 'vue'\n",
                    "const { foo } = defineProps(['foo'])\n",
                    "function useLocal(foo) { watch(foo, () => {}); foo++ }",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(shadowed["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_rewrites_props_destructure_references() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo, bar: baz, 'foo.bar': fooBar } = defineProps({ foo: String, bar: Number, 'foo.bar': Boolean })\n",
                    "const message = foo + baz\n",
                    "const payload = { foo, baz, fooBar }\n",
                    "function read(foo) { return foo + baz }\n",
                    "console.log(message, payload, fooBar)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(!content.contains("const { foo, bar: baz, 'foo.bar': fooBar }"));
        assert!(content.contains("const message = __props.foo + __props.bar"));
        assert!(content.contains(
            r#"const payload = { foo: __props.foo, baz: __props.bar, fooBar: __props["foo.bar"] }"#
        ));
        assert!(content.contains("function read(foo) { return foo + __props.bar }"));
        assert!(content.contains(r#"console.log(message, payload, __props["foo.bar"])"#));
        assert_eq!(compiled["propsAliases"]["baz"], json!("bar"));
        assert_eq!(compiled["propsAliases"]["fooBar"], json!("foo.bar"));
        assert!(compiled["propsAliases"].get("foo").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_generates_props_destructure_rest_proxy() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const { foo, bar: baz, ...rest } = defineProps(['foo', 'bar', 'baz'])\n",
                    "const read = foo + baz + rest.baz",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(r#"const rest = _createPropsRestProxy(__props, ["foo","bar"])"#));
        assert!(content.contains("const read = __props.foo + __props.bar + rest.baz"));
        assert!(!content.contains("const { foo, bar: baz, ...rest }"));
        assert!(!content.contains("defineProps"));
        assert_eq!(
            compiled["bindings"]["rest"].as_str(),
            Some("setup-reactive-const")
        );
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_inlines_template_render() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { ref } from 'vue'\n",
                    "import ChildComp from './ChildComp.vue'\n",
                    "const count = ref(0)\n",
                    "const { title: heading } = defineProps(['title'])",
                    "</script>",
                    "<template><div>{{ count }} {{ heading }}</div><ChildComp /></template>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "inlineTemplate": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("toDisplayString as _toDisplayString"));
        assert!(content.contains("return (_ctx, _cache) => {"));
        assert!(content.contains("count.value"));
        assert!(content.contains("_toDisplayString(__props.title)"));
        assert!(content.contains("_createVNode(ChildComp)"));
        assert!(!content.contains("const __returned__"));
        assert_eq!(compiled["bindings"]["heading"], json!("props-aliased"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_inlines_ssr_template_render() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { ref } from 'vue'\n",
                    "const count = ref(0)",
                    "</script>",
                    "<template><div>{{ count }}</div></template>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "id": "xxxxxxxx",
                    "inlineTemplate": true,
                    "templateOptions": {
                        "ssr": true
                    }
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(content.contains("__ssrInlineRender: true,"));
        assert!(content.contains("return (_ctx, _push, _parent, _attrs) => {"));
        assert!(content.contains("_ssrInterpolate(count.value)"));
        assert!(!content.contains("const __returned__"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_rewrites_top_level_await_runtime_module() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const value = await Promise.resolve(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "templateOptions": {
                        "compilerOptions": {
                            "runtimeModuleName": "npm:vue"
                        }
                    }
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content
            .starts_with("import { withAsyncContext as _withAsyncContext } from \"npm:vue\"\n"));
        assert!(content.contains("async setup("));
        assert!(content.contains("let __temp, __restore"));
        assert!(content.contains("_withAsyncContext(() => Promise.resolve(1))"));
        assert!(content.contains("const __returned__ = { value }"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_template_used_import_getters() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { FooBar, FooBaz, vMyDir } from './x'\n",
                    "import { ref } from 'vue'\n",
                    "const local = ref(0)",
                    "</script>",
                    "<template>",
                    "<FooBaz />",
                    "<foo-bar />",
                    "<div v-my-dir>{{ FooBar }}</div>",
                    "</template>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(
            "const __returned__ = { local, get FooBar() { return FooBar }, get FooBaz() { return FooBaz }, get vMyDir() { return vMyDir }, ref }"
        ));
        assert_eq!(compiled["bindings"]["FooBar"], json!("setup-maybe-ref"));
        assert_eq!(compiled["bindings"]["FooBaz"], json!("setup-maybe-ref"));
        assert_eq!(compiled["bindings"]["vMyDir"], json!("setup-maybe-ref"));
        assert_eq!(compiled["bindings"]["ref"], json!("setup-const"));
        assert_eq!(compiled["bindings"]["local"], json!("setup-ref"));
        assert_eq!(compiled["imports"]["FooBar"]["imported"], json!("FooBar"));
        assert_eq!(compiled["imports"]["FooBar"]["local"], json!("FooBar"));
        assert_eq!(compiled["imports"]["FooBar"]["source"], json!("./x"));
        assert_eq!(compiled["imports"]["FooBar"]["isType"], json!(false));
        assert_eq!(compiled["imports"]["FooBar"]["isFromSetup"], json!(true));
        assert_eq!(
            compiled["imports"]["FooBar"]["isUsedInTemplate"],
            json!(true)
        );
        assert_eq!(compiled["imports"]["FooBaz"]["imported"], json!("FooBaz"));
        assert_eq!(compiled["imports"]["FooBaz"]["local"], json!("FooBaz"));
        assert_eq!(compiled["imports"]["FooBaz"]["source"], json!("./x"));
        assert_eq!(compiled["imports"]["FooBaz"]["isType"], json!(false));
        assert_eq!(compiled["imports"]["FooBaz"]["isFromSetup"], json!(true));
        assert_eq!(
            compiled["imports"]["FooBaz"]["isUsedInTemplate"],
            json!(true)
        );
        assert_eq!(
            compiled["imports"]["vMyDir"]["isUsedInTemplate"],
            json!(true)
        );
        assert_eq!(compiled["imports"]["ref"]["source"], json!("vue"));
        assert_eq!(compiled["imports"]["ref"]["isFromSetup"], json!(true));
        assert_eq!(compiled["imports"]["ref"]["isUsedInTemplate"], json!(false));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_props_destructure_defaults() {
        let runtime = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "const external = 'x'\n",
                    "const { foo = 1, bar = {}, func = () => {}, ext = external, 'foo:bar': fooBar = 'foo-bar' } = defineProps(['foo', 'bar', 'func', 'ext', 'foo:bar'])",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let content = runtime["content"].as_str().unwrap_or_default();
        assert!(runtime["errors"].as_array().unwrap().is_empty());
        assert!(content.contains(
            "props: /*@__PURE__*/_mergeDefaults(['foo', 'bar', 'func', 'ext', 'foo:bar'], {"
        ));
        assert!(content.contains("bar: () => ({})"));
        assert!(content.contains("func: () => {}, __skip_func: true"));
        assert!(content.contains("ext: external, __skip_ext: true"));
        assert!(content.contains(r#""foo:bar": 'foo-bar'"#));

        let typed = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo = 1, bar = {}, func = () => {} } = defineProps<{ foo?: number, bar?: object, func?: () => void }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        let content = typed["content"].as_str().unwrap_or_default();
        assert!(typed["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: Number, required: false, default: 1 }"));
        assert!(content.contains("bar: { type: Object, required: false, default: () => ({}) }"));
        assert!(content.contains("func: { type: Function, required: false, default: () => {} }"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_props_destructure_default_type_errors() {
        let mismatch = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo = 'hello' } = defineProps<{ foo?: number }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(mismatch["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().is_some_and(|error| error
                .contains("Default value of prop \"foo\" does not match declared type."))));

        let matching = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const { foo = 1, bar = 'ok' } = defineProps<{ foo?: number, bar?: string }>()",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");
        assert!(matching["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_define_options() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { defineOptions, ref } from 'vue'\n",
                    "defineOptions({ name: 'FooApp', inheritAttrs: false })\n",
                    "const count = ref(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("import { ref } from 'vue'"));
        assert!(content.contains(
            "export default /*@__PURE__*/Object.assign({ name: 'FooApp', inheritAttrs: false }, {"
        ));
        assert!(content.contains("__name: 'FooBar',"));
        assert!(content.contains("const __returned__ = { count, ref }"));
        assert!(!content.contains("defineOptions"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert!(compiled["bindings"].get("defineOptions").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_generates_define_model() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup>",
                    "import { defineModel, ref } from 'vue'\n",
                    "defineProps({ foo: String })\n",
                    "defineEmits(['change'])\n",
                    "const count = defineModel({ default: 0 })\n",
                    "const title = defineModel('title')\n",
                    "const other = ref(1)",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content
            .contains("import { useModel as _useModel, mergeModels as _mergeModels } from 'vue'"));
        assert!(content.contains("import { ref } from 'vue'"));
        assert!(content.contains("props: /*@__PURE__*/_mergeModels({ foo: String }, {"));
        assert!(content.contains("\"modelValue\": { default: 0 },"));
        assert!(content.contains("\"title\": {},"));
        assert!(content.contains(
            "emits: /*@__PURE__*/_mergeModels(['change'], [\"update:modelValue\", \"update:title\"]),"
        ));
        assert!(content.contains(r#"const count = _useModel(__props, "modelValue")"#));
        assert!(content.contains("const title = _useModel(__props, 'title')"));
        assert!(!content.contains("defineModel"));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["modelValue"], json!("props"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["title"], json!("setup-ref"));
        assert!(compiled["bindings"].get("defineModel").is_none());
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_infers_define_model_types() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const modelValue = defineModel<boolean | string>()\n",
                    "const count = defineModel<number>('count')\n",
                    "const any = defineModel<any | boolean>('any')",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(content.contains("\"count\": { type: Number },"));
        assert!(content.contains("\"any\": { type: Boolean, skipCheck: true },"));
        assert!(
            content.contains("emits: [\"update:modelValue\", \"update:count\", \"update:any\"],")
        );
        assert!(content
            .contains(r#"const modelValue = _useModel<boolean | string>(__props, "modelValue")"#));
        assert!(content.contains("const count = _useModel<number>(__props, 'count')"));
        assert_eq!(compiled["bindings"]["modelValue"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["any"], json!("setup-ref"));

        let prod = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const modelValue = defineModel<boolean>()\n",
                    "const fn = defineModel<() => void>('fn')\n",
                    "const fnWithDefault = defineModel<() => void>('fnWithDefault', { default: () => null })\n",
                    "const str = defineModel<string>('str')",
                    "</script>"
                ),
                "filename": "FooBar.vue",
                "options": {
                    "isProd": true
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = prod["content"].as_str().unwrap_or_default();
        assert!(prod["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("\"modelValue\": { type: Boolean },"));
        assert!(content.contains("\"fn\": {},"));
        assert!(
            content.contains("\"fnWithDefault\": { type: Function, ...{ default: () => null } },")
        );
        assert!(content.contains("\"str\": {},"));
        assert_eq!(prod["bindings"]["modelValue"], json!("setup-ref"));
        assert_eq!(prod["bindings"]["fn"], json!("setup-ref"));
        assert_eq!(prod["bindings"]["fnWithDefault"], json!("setup-ref"));
        assert_eq!(prod["bindings"]["str"], json!("setup-ref"));
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_type_resolution_deps() {
        let dir =
            std::env::temp_dir().join(format!("vuec-node-bridge-deps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("props.ts"), "export type Props = { foo: string }")
            .expect("write props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './props'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("props.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_resolve_type_projects_props_calls_and_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-resolve-type-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("props.ts"),
            "export type Props = { foo: number; bar?: string; (e: 'save'): void }",
        )
        .expect("write props");

        let filename = dir.join("Comp.vue");
        let resolved = dispatch(
            "sfc.resolveType",
            json!({
                "code": "import type { Props } from './props'\ndefineProps<Props>()",
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 resolveType");

        let expected_dep = dir.join("props.ts").to_string_lossy().replace('\\', "/");
        assert!(resolved["errors"].as_array().unwrap().is_empty());
        assert_eq!(resolved["props"]["foo"], json!(["Number"]));
        assert_eq!(resolved["props"]["bar"], json!(["String"]));
        assert_eq!(resolved["raw"]["props"]["bar"]["optional"], json!(true));
        assert_eq!(resolved["calls"].as_array().unwrap().len(), 1);
        assert_eq!(resolved["deps"], json!([expected_dep]));

        let failed = dispatch(
            "sfc.resolveType",
            json!({
                "code": "defineProps<Missing>()",
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 resolveType failed projection");
        assert!(failed["errors"].as_array().unwrap().iter().any(|error| {
            error
                .as_str()
                .is_some_and(|error| error.contains("Unresolvable type reference"))
        }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_extract_prop_types_return_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-extract-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("upload.ts"),
            concat!(
                "import type { PropType } from 'vue'\n",
                "export interface UploadFile<T> { raw: T }\n",
                "export declare function uploadProps<T>(): {\n",
                "  fileList: { type: PropType<UploadFile<T>[]>, default: UploadFile<T>[] }\n",
                "}\n"
            ),
        )
        .expect("write upload props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { uploadProps } from './upload'\n",
                    "declare const props: () => {\n",
                    "  active: { type: BooleanConstructor, required: true }\n",
                    "}\n",
                    "type Props = Partial<import('vue').ExtractPropTypes<ReturnType<typeof props>>> & import('vue').ExtractPropTypes<ReturnType<typeof uploadProps>>\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("upload.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("active: { type: Boolean, required: false }"));
        assert!(content.contains("fileList: { type: Array, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_runtime_props_object_extract_prop_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-runtime-props-object-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("user.ts"), "export interface User { id: string }")
            .expect("write user type");
        std::fs::write(
            dir.join("props.ts"),
            concat!(
                "import type { PropType } from 'vue'\n",
                "import type { User } from './user'\n",
                "export const props = {\n",
                "  name: String,\n",
                "  active: { type: Boolean, required: true },\n",
                "  score: { type: [Number, String] },\n",
                "  user: Object as PropType<User>\n",
                "}\n"
            ),
        )
        .expect("write runtime props");
        std::fs::write(
            dir.join("default-props.ts"),
            concat!(
                "const props = {\n",
                "  flag: Boolean,\n",
                "  created: { type: Date, default: () => new Date() }\n",
                "}\n",
                "export { props as default }\n"
            ),
        )
        .expect("write default runtime props");
        std::fs::write(
            dir.join("direct-default-props.ts"),
            concat!(
                "import type { PropType } from 'vue'\n",
                "import type { User } from './user'\n",
                "export default {\n",
                "  direct: { type: String, required: true },\n",
                "  owner: Object as PropType<User>,\n",
                "  mode: { type: [Boolean, Number] }\n",
                "}\n"
            ),
        )
        .expect("write direct default runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { props as namedProps } from './props'\n",
                    "import defaultProps from './default-props'\n",
                    "import directDefaultProps from './direct-default-props'\n",
                    "type Props =\n",
                    "  ExtractPropTypes<typeof namedProps> &\n",
                    "  Partial<ExtractPropTypes<typeof defaultProps>> &\n",
                    "  ExtractPropTypes<typeof directDefaultProps>\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_deps = json!([
            dir.join("default-props.ts")
                .to_string_lossy()
                .replace('\\', "/"),
            dir.join("direct-default-props.ts")
                .to_string_lossy()
                .replace('\\', "/"),
            dir.join("props.ts").to_string_lossy().replace('\\', "/"),
            dir.join("user.ts").to_string_lossy().replace('\\', "/")
        ]);
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("name: { type: String, required: false }"));
        assert!(content.contains("active: { type: Boolean, required: true }"));
        assert!(content.contains("score: { type: [Number, String], required: false }"));
        assert!(content.contains("user: { type: Object, required: false }"));
        assert!(content.contains("flag: { type: Boolean, required: false }"));
        assert!(content.contains("created: { type: Date, required: false }"));
        assert!(content.contains("direct: { type: String, required: true }"));
        assert!(content.contains("owner: { type: Object, required: false }"));
        assert!(content.contains("mode: { type: [Boolean, Number], required: false }"));
        assert_eq!(compiled["deps"], expected_deps);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_generic_utility_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-generic-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export type Props<T> = Readonly<Partial<T>>\nexport type Base = { ext: string }",
        )
        .expect("write generic props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, Base } from './types'\n",
                    "defineProps<Props<Base>>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ext: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_mapped_template_literal_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-mapped-template-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "type Breakpoints = 'sm' | 'md'\n",
                "export type Props<T extends string, V> = {\n",
                "  [K in Breakpoints as `${T}${Capitalize<K>}`]?: V\n",
                "}"
            ),
        )
        .expect("write mapped props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props<'cols', number>>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("colsSm: { type: Number, required: false }"));
        assert!(content.contains("colsMd: { type: Number, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_record_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-record-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "type Breakpoints = 'sm' | 'md'\n",
                "export type Props<T extends string, V> =\n",
                "  Record<`${T}${Capitalize<Breakpoints>}`, V> &\n",
                "  Partial<Record<Uppercase<Breakpoints>, string>>"
            ),
        )
        .expect("write record props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props<'cols', number>>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("colsSm: { type: Number, required: true }"));
        assert!(content.contains("colsMd: { type: Number, required: true }"));
        assert!(content.contains("SM: { type: String, required: false }"));
        assert!(content.contains("MD: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_indexed_access_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-indexed-access-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Base = { name: string; count?: number; active: boolean }\n",
                "export type MethodBase = { method(): void; run: () => void; value: string }\n",
                "export type A = (string | number)[]\n",
                "export type TT = [foo: 1, bar: 'foo']\n",
                "export type ValueOf<T, K extends keyof T> = T[K]\n",
                "export type Props = {\n",
                "  label: ValueOf<Base, 'name'>\n",
                "  scalar: Base['name' | 'count']\n",
                "  active: Base['active']\n",
                "  method: MethodBase['method']\n",
                "  callable: MethodBase['run']\n",
                "  methodOrCallable: MethodBase['method'] | MethodBase['run']\n",
                "  methodOrLabel: MethodBase['method'] | MethodBase['value']\n",
                "  arrayItem: A[number]\n",
                "  tupleItem: TT[number]\n",
                "}\n",
                "export type ModelValue = A[number] | TT[number] | MethodBase['method'] | MethodBase['run']"
            ),
        )
        .expect("write indexed access props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("scalar: { type: [String, Number], required: true }"));
        assert!(content.contains("active: { type: Boolean, required: true }"));
        assert!(content.contains("method: { type: null, required: true }"));
        assert!(content.contains("callable: { type: Function, required: true }"));
        assert!(content
            .contains("methodOrCallable: { type: Function, required: true, skipCheck: true }"));
        assert!(content.contains("methodOrLabel: { type: null, required: true }"));
        assert!(content.contains("arrayItem: { type: [String, Number], required: true }"));
        assert!(content.contains("tupleItem: { type: [Number, String], required: true }"));
        assert!(content
            .contains("\"modelValue\": { type: [String, Number, Function], skipCheck: true },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_parameter_tuple_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-parameter-tuple-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Fn = (value: string, count: number, active?: boolean) => void\n",
                "export type Ctor = new (name: string, flags: boolean[]) => object\n",
                "export type Props = {\n",
                "  first: Parameters<Fn>[0]\n",
                "  anyParam: Parameters<Fn>[number]\n",
                "  ctorFirst: ConstructorParameters<Ctor>[0]\n",
                "  ctorAny: ConstructorParameters<Ctor>[number]\n",
                "  inlineParam: Parameters<(files: File[], done: () => void) => void>[number]\n",
                "}\n",
                "export type ModelValue = Parameters<Fn>[number] | ConstructorParameters<Ctor>[number]"
            ),
        )
        .expect("write parameter tuple props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("first: { type: String, required: true }"));
        assert!(content.contains("anyParam: { type: [String, Number, Boolean], required: true }"));
        assert!(content.contains("ctorFirst: { type: String, required: true }"));
        assert!(content.contains("ctorAny: { type: [String, Array], required: true }"));
        assert!(content.contains("inlineParam: { type: [Array, Function], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Boolean, Array] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_signature_parameter_tuples_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-signature-parameter-tuple-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Callable = {\n",
                "  (value: string, count: number): void\n",
                "  (active: boolean): void\n",
                "}\n",
                "export interface InterfaceCallable {\n",
                "  (name: string, flags: boolean[]): void\n",
                "}\n",
                "export type Newable = {\n",
                "  new (id: number, done: () => void): object\n",
                "}\n",
                "export interface InterfaceNewable {\n",
                "  new (label: string, enabled: boolean): object\n",
                "}\n",
                "export type Props = {\n",
                "  callAny: Parameters<Callable>[number]\n",
                "  callFirst: Parameters<InterfaceCallable>[0]\n",
                "  newAny: ConstructorParameters<Newable>[number]\n",
                "  newSecond: ConstructorParameters<InterfaceNewable>[1]\n",
                "}\n",
                "export type ModelValue = Parameters<Callable>[number] | ",
                "ConstructorParameters<InterfaceNewable>[number]"
            ),
        )
        .expect("write signature parameter tuple props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("callAny: { type: [String, Boolean, Number], required: true }"));
        assert!(content.contains("callFirst: { type: String, required: true }"));
        assert!(content.contains("newAny: { type: [Number, Function], required: true }"));
        assert!(content.contains("newSecond: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_extends_signature_parameter_tuples_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-extends-signature-parameter-tuple-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export interface Callable extends BaseCallable {\n",
                "  (active: boolean): void\n",
                "}\n",
                "export interface BaseCallable {\n",
                "  (name: string, count: number): void\n",
                "}\n",
                "export interface Newable extends BaseNewable {\n",
                "  new (label: string): object\n",
                "}\n",
                "export interface BaseNewable {\n",
                "  new (id: number, done: () => void): object\n",
                "}\n",
                "export type Props = {\n",
                "  callAny: Parameters<Callable>[number]\n",
                "  callSecond: Parameters<Callable>[1]\n",
                "  newAny: ConstructorParameters<Newable>[number]\n",
                "  newSecond: ConstructorParameters<Newable>[1]\n",
                "}\n",
                "export type ModelValue = Parameters<Callable>[number] | ",
                "ConstructorParameters<Newable>[number]"
            ),
        )
        .expect("write extends signature parameter tuple props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("callAny: { type: [Boolean, String, Number], required: true }"));
        assert!(content.contains("callSecond: { type: Number, required: true }"));
        assert!(content.contains("newAny: { type: [String, Number, Function], required: true }"));
        assert!(content.contains("newSecond: { type: Function, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String, Number, Function] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_runtime_utility_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-runtime-utility-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type MaybeText = string | null\n",
                "export type Props = {\n",
                "  label: NonNullable<MaybeText>\n",
                "  extracted: Extract<string | number | boolean, number | boolean>\n",
                "  excluded: Exclude<string | number, number>\n",
                "}\n",
                "export type ModelValue =\n",
                "  NonNullable<string | null> |\n",
                "  Extract<number | boolean, boolean> |\n",
                "  Exclude<string | number, number>"
            ),
        )
        .expect("write runtime utility props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("extracted: { type: [Number, Boolean], required: true }"));
        assert!(content.contains("excluded: { type: [String, Number], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean, Number] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export declare function makeLabel(): string\n",
                "export declare const makeCount: () => number\n",
                "export type BooleanFactory = () => boolean\n",
                "export type Callable = {\n",
                "  (value: string): Date\n",
                "  (value: number): Error\n",
                "}\n",
                "export interface InterfaceFactory {\n",
                "  (active: boolean): string[]\n",
                "}\n",
                "export interface ExtendedFactory extends InterfaceFactory {\n",
                "  (value: number): boolean\n",
                "}\n",
                "export type Props = {\n",
                "  label: ReturnType<typeof makeLabel>\n",
                "  count: ReturnType<typeof makeCount>\n",
                "  flag: ReturnType<BooleanFactory>\n",
                "  mixed: ReturnType<Callable>\n",
                "  list: ReturnType<InterfaceFactory>\n",
                "  extended: ReturnType<ExtendedFactory>\n",
                "}\n",
                "export type ModelValue = ",
                "ReturnType<typeof makeLabel> | ReturnType<BooleanFactory>"
            ),
        )
        .expect("write return type props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("mixed: { type: [Date, Error], required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("extended: { type: [Boolean, Array], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_default_function_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-default-function-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("named.ts"),
            concat!(
                "export default function makeDefault(): string { return '' }\n",
                "export function makeCount(): number { return 1 }"
            ),
        )
        .expect("write named default function type");
        std::fs::write(
            dir.join("anonymous.ts"),
            "export default function(): boolean { return true }",
        )
        .expect("write anonymous default function type");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import makeDefault, { makeCount } from './named'\n",
                    "import makeFlag from './anonymous'\n",
                    "type Props = {\n",
                    "  label: ReturnType<typeof makeDefault>\n",
                    "  count: ReturnType<typeof makeCount>\n",
                    "  flag: ReturnType<typeof makeFlag>\n",
                    "}\n",
                    "defineProps<Props>()\n",
                    "defineModel<",
                    "ReturnType<typeof makeDefault> | ",
                    "ReturnType<typeof makeCount> | ",
                    "ReturnType<typeof makeFlag>",
                    ">()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["named.ts", "anonymous.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Boolean] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_function_value_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-function-value-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("factories.ts"),
            concat!(
                "export type Label = string\n",
                "export type Count = number\n",
                "export type Flag = boolean\n",
                "export const makeLabel = (): Label => ''\n",
                "export const makeCount: () => Count = () => 1\n",
                "export const makeFlag = function(): Flag { return true }"
            ),
        )
        .expect("write function value factories");
        std::fs::write(
            dir.join("arrow-default.ts"),
            "export default ((): Date => new Date())",
        )
        .expect("write default arrow function value");
        std::fs::write(
            dir.join("function-default.ts"),
            "export default (function(): Error { return new Error() })",
        )
        .expect("write default function expression value");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import makeDate from './arrow-default'\n",
                    "import makeError from './function-default'\n",
                    "import { makeLabel, makeCount, makeFlag } from './factories'\n",
                    "type Props = {\n",
                    "  label: ReturnType<typeof makeLabel>\n",
                    "  count: ReturnType<typeof makeCount>\n",
                    "  flag: ReturnType<typeof makeFlag>\n",
                    "  date: ReturnType<typeof makeDate>\n",
                    "  error: ReturnType<typeof makeError>\n",
                    "}\n",
                    "defineProps<Props>()\n",
                    "defineModel<",
                    "ReturnType<typeof makeLabel> | ",
                    "ReturnType<typeof makeCount> | ",
                    "ReturnType<typeof makeFlag> | ",
                    "ReturnType<typeof makeDate> | ",
                    "ReturnType<typeof makeError>",
                    ">()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["factories.ts", "arrow-default.ts", "function-default.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("date: { type: Date, required: true }"));
        assert!(content.contains("error: { type: Error, required: true }"));
        assert!(
            content.contains("\"modelValue\": { type: [String, Number, Boolean, Date, Error] },")
        );
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_unannotated_return_type_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-unannotated-return-type-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("factories.ts"),
            concat!(
                "export function makeLabel() { return 'label' }\n",
                "export const makeCount = () => 1\n",
                "export const makeFlag = function() { return true }\n",
                "export const makeList = () => []\n",
                "export function makeBox() { return { label: 'box' } }"
            ),
        )
        .expect("write unannotated factories");
        std::fs::write(
            dir.join("date.ts"),
            "export default function makeDate() { return new Date() }",
        )
        .expect("write default unannotated function");
        std::fs::write(
            dir.join("error.ts"),
            "export default (function() { return new Error('x') })",
        )
        .expect("write default unannotated function expression");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import makeDate from './date'\n",
                    "import makeError from './error'\n",
                    "import { makeLabel, makeCount, makeFlag, makeList, makeBox } from './factories'\n",
                    "type Props = {\n",
                    "  label: ReturnType<typeof makeLabel>\n",
                    "  count: ReturnType<typeof makeCount>\n",
                    "  flag: ReturnType<typeof makeFlag>\n",
                    "  list: ReturnType<typeof makeList>\n",
                    "  box: ReturnType<typeof makeBox>\n",
                    "  made: ReturnType<typeof import('./factories').makeFlag>\n",
                    "  created: ReturnType<typeof makeDate>\n",
                    "  error: ReturnType<typeof makeError>\n",
                    "}\n",
                    "defineProps<Props>()\n",
                    "defineModel<",
                    "ReturnType<typeof makeLabel> | ",
                    "ReturnType<typeof makeCount> | ",
                    "ReturnType<typeof makeFlag> | ",
                    "ReturnType<typeof makeList> | ",
                    "ReturnType<typeof makeBox> | ",
                    "ReturnType<typeof makeDate> | ",
                    "ReturnType<typeof makeError>",
                    ">()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["factories.ts", "date.ts", "error.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("box: { type: Object, required: true }"));
        assert!(content.contains("made: { type: Boolean, required: true }"));
        assert!(content.contains("created: { type: Date, required: true }"));
        assert!(content.contains("error: { type: Error, required: true }"));
        assert!(content.contains(
            "\"modelValue\": { type: [String, Number, Boolean, Array, Object, Date, Error] },"
        ));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_builtin_wrapper_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-builtin-wrapper-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Props = {\n",
                "  list: ReadonlyArray<string>\n",
                "  params: Parameters<(value: string) => void>\n",
                "  map: ReadonlyMap<string, number>\n",
                "  set: ReadonlySet<string>\n",
                "  err: Error\n",
                "  maybe: MaybeRef<string[]>\n",
                "  getter: MaybeRefOrGetter<boolean>\n",
                "}\n",
                "export type ModelValue =\n",
                "  ReadonlyArray<string> |\n",
                "  ReadonlyMap<string, number> |\n",
                "  ReadonlySet<string> |\n",
                "  Error |\n",
                "  MaybeRefOrGetter<boolean> |\n",
                "  Parameters<() => void>"
            ),
        )
        .expect("write builtin wrapper props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("params: { type: Array, required: true }"));
        assert!(content.contains("map: { type: Map, required: true }"));
        assert!(content.contains("set: { type: Set, required: true }"));
        assert!(content.contains("err: { type: Error, required: true }"));
        assert!(content.contains("maybe: { type: [Object, Array], required: true }"));
        assert!(content.contains("getter: { type: [Object, Function, Boolean], required: true }"));
        assert!(content.contains(
            "\"modelValue\": { type: [Array, Map, Set, Error, Object, Function, Boolean] },"
        ));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_mapped_identity_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-mapped-identity-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type RuntimeMirror<T> = { [K in keyof T]: T[K] }\n",
                "export type Props = {\n",
                "  label: RuntimeMirror<string | number>\n",
                "  boxed: RuntimeMirror<{ value: boolean }>\n",
                "  list: RuntimeMirror<ReadonlyArray<string>>\n",
                "}\n",
                "export type ModelValue = RuntimeMirror<string | boolean>"
            ),
        )
        .expect("write mapped identity runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("label: { type: [String, Number], required: true }"));
        assert!(content.contains("boxed: { type: Object, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_static_conditional_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-conditional-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Runtime<T> = ",
                "T extends 'text' ? string : ",
                "T extends 'count' ? number : boolean\n",
                "export type Props = {\n",
                "  directTrue: 'on' extends 'on' ? boolean : string\n",
                "  directFalse: 'off' extends 'on' ? boolean : string\n",
                "  text: Runtime<'text'>\n",
                "  count: Runtime<'count'>\n",
                "  active: Runtime<'active'>\n",
                "  unresolved: Runtime<'text' | 'count'>\n",
                "}\n",
                "export type ModelValue = Runtime<'text'> | Runtime<'count'> | Runtime<'active'>"
            ),
        )
        .expect("write conditional runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("directTrue: { type: Boolean, required: true }"));
        assert!(content.contains("directFalse: { type: String, required: true }"));
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("active: { type: Boolean, required: true }"));
        assert!(content.contains("unresolved: { type: null, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Boolean] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_bigint_literal_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-bigint-literal-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Big = 1n\n",
                "export type Props = {\n",
                "  literal: 1n\n",
                "  union: 1n | 'text'\n",
                "  alias: Big\n",
                "  keyword: bigint\n",
                "}\n",
                "export type ModelValue = 1n | 'text'"
            ),
        )
        .expect("write bigint literal runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("literal: { type: Number, required: true }"));
        assert!(content.contains("union: { type: [Number, String], required: true }"));
        assert!(content.contains("alias: { type: Number, required: true }"));
        assert!(content.contains("keyword: { type: null, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_type_operator_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-type-operator-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Base = { name: string; 1: boolean }\n",
                "export type Props = {\n",
                "  readonlyList: readonly string[]\n",
                "  objectKeys: keyof Base\n",
                "  literalKeys: keyof { [index: number]: string; label: string }\n",
                "  arrayKeys: keyof ReadonlyArray<string>\n",
                "  anyKeys: keyof any\n",
                "  pickedKeys: keyof Pick<Base, 'name'>\n",
                "}\n",
                "export type ModelValue = readonly boolean[] | keyof any"
            ),
        )
        .expect("write type operator runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("readonlyList: { type: Array, required: true }"));
        assert!(content.contains("objectKeys: { type: [String, Number], required: true }"));
        assert!(content.contains("literalKeys: { type: [Number, String], required: true }"));
        assert!(content.contains("arrayKeys: { type: [String, Number], required: true }"));
        assert!(content.contains("anyKeys: { type: [String, Number, Symbol], required: true }"));
        assert!(content.contains("pickedKeys: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Array, String, Number, Symbol] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_type_query_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-type-query-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export declare const text: string\n",
                "export declare const flag: boolean\n",
                "export declare const list: string[]\n",
                "export declare const boxed: { id: string }\n",
                "export type Props = {\n",
                "  text: typeof text\n",
                "  flag: typeof flag\n",
                "  list: typeof list\n",
                "  keys: keyof typeof boxed\n",
                "}\n",
                "export type ModelValue = typeof flag | typeof list"
            ),
        )
        .expect("write type query runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("flag: { type: Boolean, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("keys: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, Array] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_qualified_type_query_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-qualified-type-query-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("values.ts"),
            concat!(
                "export declare const text: string\n",
                "export declare const boxed: { id: string }\n",
                "export declare const list: string[]\n"
            ),
        )
        .expect("write type query values");
        std::fs::write(dir.join("facade.ts"), "export * from './values'")
            .expect("write type query facade");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "import * as Values from './facade'\n",
                "export type Props = {\n",
                "  text: typeof Values.text\n",
                "  keys: keyof typeof Values.boxed\n",
                "  list: typeof Values.list\n",
                "}\n",
                "export type ModelValue = typeof Values.text | typeof Values.list"
            ),
        )
        .expect("write qualified type query runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["types.ts", "facade.ts", "values.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("keys: { type: String, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Array] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_import_type_query_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-import-type-query-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("values.ts"),
            concat!(
                "export declare const text: string\n",
                "export declare const boxed: { id: string }\n",
                "export declare const list: string[]\n",
                "export declare const options: { enabled: BooleanConstructor }\n",
                "export function make(): boolean { return true }\n"
            ),
        )
        .expect("write import type query values");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Props = ExtractPropTypes<typeof import('./values').options> & {\n",
                "  text: typeof import('./values').text\n",
                "  keys: keyof typeof import('./values').boxed\n",
                "  list: typeof import('./values').list\n",
                "  made: ReturnType<typeof import('./values').make>\n",
                "}\n",
                "export type ModelValue = ",
                "typeof import('./values').text | ",
                "ReturnType<typeof import('./values').make> | ",
                "typeof import('./values').list"
            ),
        )
        .expect("write import type query runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["types.ts", "values.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("enabled: { type: Boolean, required: false }"));
        assert!(content.contains("text: { type: String, required: true }"));
        assert!(content.contains("keys: { type: String, required: true }"));
        assert!(content.contains("list: { type: Array, required: true }"));
        assert!(content.contains("made: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Boolean, Array] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_signature_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-signature-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Callable = { (): string }\n",
                "export type Constructable = { new (): object }\n",
                "export interface InterfaceMixed {\n",
                "  new (): object\n",
                "  value: number\n",
                "}\n",
                "export type Props = {\n",
                "  call: Callable\n",
                "  ctor: Constructable\n",
                "  ifaceMixed: InterfaceMixed\n",
                "}\n",
                "export type ModelValue = Callable | InterfaceMixed"
            ),
        )
        .expect("write signature runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("call: { type: Function, required: true }"));
        assert!(content.contains("ctor: { type: Function, required: true }"));
        assert!(content.contains("ifaceMixed: { type: [Function, Object], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Function, Object] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_intersection_runtime_types_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-intersection-runtime-types-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Callable = { (): string }\n",
                "export type Box = { value: number }\n",
                "export type Props = {\n",
                "  scalar: string & number\n",
                "  callableBox: Callable & Box\n",
                "  maybe: any | boolean\n",
                "  unknown: any\n",
                "}\n",
                "export type ModelValue = (string & number) | (Callable & Box)"
            ),
        )
        .expect("write intersection runtime props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "defineProps<Props>()\n",
                    "defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("scalar: { type: [String, Number], required: true }"));
        assert!(content.contains("callableBox: { type: [Function, Object], required: true }"));
        assert!(content.contains("maybe: { type: Boolean, required: true, skipCheck: true }"));
        assert!(content.contains("unknown: { type: null, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number, Function, Object] },"));
        assert!(!content.contains("Unknown"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_merges_external_duplicate_union_intersection_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-duplicate-union-intersection-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export type Left = { shared: string; unknownBool: any; left?: boolean }\n",
                "export type Right = { shared?: number; unknownBool: boolean; right: Function }\n",
                "export type Props = Left & Right & ",
                "({ variant: string } | { variant?: boolean }) & ",
                "({ maybe: any } | { maybe: boolean })"
            ),
        )
        .expect("write duplicate props types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert_eq!(content.matches("shared: {").count(), 1);
        assert_eq!(content.matches("variant: {").count(), 1);
        assert_eq!(content.matches("maybe: {").count(), 1);
        assert!(content.contains("shared: { type: [String, Number], required: false }"));
        assert!(content.contains("unknownBool: { type: Boolean, required: true }"));
        assert!(content.contains("left: { type: Boolean, required: false }"));
        assert!(content.contains("right: { type: Function, required: true }"));
        assert!(content.contains("variant: { type: [String, Boolean], required: false }"));
        assert!(content.contains("maybe: { type: Boolean, required: true, skipCheck: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_interface_extends_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export interface Base { ext?: string }\nexport interface Props extends Base { local: number }",
        )
        .expect("write interface props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ext: { type: String, required: false }"));
        assert!(content.contains("local: { type: Number, required: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_forward_interface_extends_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-forward-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export interface Props extends Base { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write interface props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("ext: { type: String, required: false }"));
        assert!(content.contains("local: { type: Number, required: true }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_reports_failed_interface_extends_and_honors_vue_ignore_deps()
    {
        let unresolved_dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-failed-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&unresolved_dir);
        std::fs::create_dir_all(&unresolved_dir).expect("create temp dir");
        std::fs::write(
            unresolved_dir.join("types.ts"),
            "import type Base from 'unknown'\nexport interface Props extends Base { local: number }",
        )
        .expect("write unresolved interface props");

        let unresolved_filename = unresolved_dir.join("Comp.vue");
        let unresolved = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": unresolved_filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let unresolved_content = unresolved["content"].as_str().unwrap_or_default();
        let unresolved_expected_dep = unresolved_dir
            .join("types.ts")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(unresolved["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| {
                error.as_str().is_some_and(|error| {
                    error.contains("Failed to resolve extends base type")
                        && error.contains("@vue-ignore")
                })
            }));
        assert!(unresolved_content.contains("local: { type: Number, required: true }"));
        assert_eq!(unresolved["deps"], json!([unresolved_expected_dep]));
        let _ = std::fs::remove_dir_all(&unresolved_dir);

        let ignored_dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-ignored-interface-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&ignored_dir);
        std::fs::create_dir_all(&ignored_dir).expect("create temp dir");
        std::fs::write(
            ignored_dir.join("types.ts"),
            "interface Base { skipped?: string }\nexport interface Props extends /*@vue-ignore*/ Base { local: number }",
        )
        .expect("write ignored interface props");

        let ignored_filename = ignored_dir.join("Comp.vue");
        let ignored = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": ignored_filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let ignored_content = ignored["content"].as_str().unwrap_or_default();
        let ignored_expected_dep = ignored_dir
            .join("types.ts")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(ignored["errors"].as_array().unwrap().is_empty());
        assert!(ignored_content.contains("local: { type: Number, required: true }"));
        assert!(!ignored_content.contains("skipped: {"));
        assert_eq!(ignored["deps"], json!([ignored_expected_dep]));
        let _ = std::fs::remove_dir_all(&ignored_dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_honors_vue_ignore_on_property_signature_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-property-signature-vue-ignore-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "type Foo = string\nexport interface Props { foo: /* @vue-ignore */ Foo; bar?: Foo }",
        )
        .expect("write ignored property signature type");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: null, required: true }"));
        assert!(content.contains("bar: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_forward_type_alias_props_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-forward-type-alias-props-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export type Props = Base & { local: number }\nexport interface Base { ext?: string }",
        )
        .expect("write type alias props");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './types'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("types.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("local: { type: Number, required: true }"));
        assert!(content.contains("ext: { type: String, required: false }"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_forward_type_alias_intersection_emits_deps()
    {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-forward-type-alias-emits-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("events.ts"),
            "export type Emits = Mid & { (e: 'local'): void }\nexport type Mid = Base & { (e: 'mid'): void }\nexport interface Base { (e: 'base'): void }",
        )
        .expect("write type alias emits");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Emits } from './events'\n",
                    "const emit = defineEmits<Emits>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("events.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("emits: [\"base\", \"mid\", \"local\"],"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_define_emits_property_syntax_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-emits-property-syntax-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("events.ts"),
            "export type Emits = { foo: []; bar: [id: number]; 'foo:bar': [] }",
        )
        .expect("write property emits");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Emits } from './events'\n",
                    "const emit = defineEmits<Emits>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("events.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("emits: [\"foo\", \"bar\", \"foo:bar\"],"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_define_emits_union_function_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-emits-union-function-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("events.ts"),
            concat!(
                "export type BaseEmit = 'change'\n",
                "export type Emit = 'some' | 'emit' | BaseEmit\n",
                "export type Emits = ",
                "((e: 'foo' | 'bar') => void) | ",
                "((e: Emit) => void) | ",
                "((e: 'another', val: string) => void)"
            ),
        )
        .expect("write union emits");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Emits } from './events'\n",
                    "const emit = defineEmits<Emits>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = dir.join("events.ts").to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content
            .contains("emits: [\"foo\", \"bar\", \"some\", \"emit\", \"change\", \"another\"],"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_re_exported_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-re-export-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("leaf.ts"), "export type Props = { foo: string }")
            .expect("write leaf");
        std::fs::write(
            dir.join("bar.ts"),
            "export { Props as PublicProps } from './leaf'",
        )
        .expect("write bar");
        std::fs::write(
            dir.join("foo.ts"),
            "export { PublicProps as Props } from './bar'",
        )
        .expect("write foo");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from './foo'\n",
                    "defineProps<Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["foo.ts", "bar.ts", "leaf.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_default_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-default-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("leaf.ts"),
            "export default interface Props { foo: string }",
        )
        .expect("write leaf");
        std::fs::write(dir.join("bar.ts"), "export { default } from './leaf'").expect("write bar");
        std::fs::write(
            dir.join("named.ts"),
            "export interface NamedProps { bar?: number }",
        )
        .expect("write named");
        std::fs::write(
            dir.join("baz.ts"),
            "export { NamedProps as default } from './named'",
        )
        .expect("write baz");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import Props from './bar'\n",
                    "import ExtraProps from './baz'\n",
                    "defineProps<Props & ExtraProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["bar.ts", "leaf.ts", "baz.ts", "named.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("bar: { type: Number, required: false }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_class_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-class-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("classes.ts"),
            "export class NamedClass {}\nexport type Props = { named: NamedClass }",
        )
        .expect("write class types");
        std::fs::write(dir.join("leaf.ts"), "export default class DefaultClass {}")
            .expect("write default class leaf");
        std::fs::write(dir.join("bar.ts"), "export { default } from './leaf'")
            .expect("write default class facade");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import DefaultClass from './bar'\n",
                    "import type { Props } from './classes'\n",
                    "class LocalClass {}\n",
                    "defineProps<{ local: LocalClass, external: Props, value: DefaultClass }>()\n",
                    "defineModel<DefaultClass>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["classes.ts", "bar.ts", "leaf.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("local: { type: Object, required: true }"));
        assert!(content.contains("external: { type: Object, required: true }"));
        assert!(content.contains("value: { type: Object, required: true }"));
        assert!(content.contains("\"modelValue\": { type: Object },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_enum_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-enum-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("enums.ts"),
            "export enum Kind { A = 'a', B = 'b' }\nexport enum Code { A = 1, B = 2 }\nexport enum Mixed { A = 'a', B = 1 }\nexport enum Auto { A, B }\nexport type Props = { kind: Kind, code?: Code, mixed: Mixed, auto: Auto }\nexport type ModelValue = Kind | Code",
        )
        .expect("write enums");
        std::fs::write(
            dir.join("facade.ts"),
            "export { Props as FacadeProps, ModelValue as FacadeModel } from './enums'",
        )
        .expect("write facade");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { FacadeProps, FacadeModel } from './facade'\n",
                    "const props = defineProps<FacadeProps>()\n",
                    "const model = defineModel<FacadeModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["enums.ts", "facade.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("kind: { type: String, required: true }"));
        assert!(content.contains("code: { type: Number, required: false }"));
        assert!(content.contains("mixed: { type: [String, Number], required: true }"));
        assert!(content.contains("auto: { type: Number, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [String, Number] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_external_merged_type_declarations() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-merged-type-declarations-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            concat!(
                "export interface Foo { a: string }\n",
                "export interface Foo { b: number }\n",
                "export namespace Bar { export type A = string }\n",
                "export namespace Bar { export type B = number }\n",
                "export namespace Baz { export type A = string }\n",
                "export interface Baz { b: number }\n",
                "export enum Kind { A = 1 }\n",
                "export enum Kind { B = 'hi' }\n",
                "export type Props = { ",
                "foo: Foo['a'], ",
                "bar: Foo['b'], ",
                "nsA: Bar.A, ",
                "nsB: Bar.B, ",
                "mixedNs: Baz.A, ",
                "mixedInterface: Baz['b'], ",
                "kind: Kind ",
                "}\n",
                "export type ModelValue = Kind"
            ),
        )
        .expect("write merged types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props, ModelValue } from './types'\n",
                    "const props = defineProps<Props>()\n",
                    "const model = defineModel<ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["types.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("bar: { type: Number, required: true }"));
        assert!(content.contains("nsA: { type: String, required: true }"));
        assert!(content.contains("nsB: { type: Number, required: true }"));
        assert!(content.contains("mixedNs: { type: String, required: true }"));
        assert!(content.contains("mixedInterface: { type: Number, required: true }"));
        assert!(content.contains("kind: { type: [Number, String], required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Number, String] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_bare_package_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let node_modules = dir.join("node_modules");
        let types_pkg = node_modules.join("vuec-bridge-types");
        let types_dist = types_pkg.join("dist");
        std::fs::create_dir_all(&types_dist).expect("create types package");
        std::fs::write(
            types_pkg.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write types package manifest");
        std::fs::write(
            types_dist.join("index.d.ts"),
            "export interface Props { root: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write types package root");
        std::fs::write(
            types_dist.join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write model type");

        let exports_pkg = node_modules.join("vuec-bridge-exports");
        std::fs::create_dir_all(exports_pkg.join("types")).expect("create exports package");
        std::fs::write(
            exports_pkg.join("package.json"),
            r#"{"exports":{"./feature":{"types":"./types/feature.d.ts","default":"./dist/feature.js"}}}"#,
        )
        .expect("write exports package manifest");
        std::fs::write(
            exports_pkg.join("types").join("feature.d.ts"),
            "export type FeatureProps = { count?: number }",
        )
        .expect("write feature type");

        let ambient_pkg = node_modules.join("@types").join("vuec-bridge-ambient");
        std::fs::create_dir_all(&ambient_pkg).expect("create @types package");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambient: string }",
        )
        .expect("write ambient type");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from 'vuec-bridge-types'\n",
                    "import type { FeatureProps } from 'vuec-bridge-exports/feature'\n",
                    "import type { AmbientProps } from 'vuec-bridge-ambient'\n",
                    "const props = defineProps<Props & FeatureProps & AmbientProps>()\n",
                    "const model = defineModel<import('vuec-bridge-types').ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            types_dist.join("index.d.ts"),
            types_dist.join("model.d.ts"),
            exports_pkg.join("types").join("feature.d.ts"),
            ambient_pkg.join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: false }"));
        assert!(content.contains("ambient: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_package_types_versions_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-types-versions-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let node_modules = dir.join("node_modules");
        let versioned_pkg = node_modules.join("vuec-bridge-typesversions");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("future").join("feature"))
            .expect("create future types");
        std::fs::create_dir_all(versioned_pkg.join("ts5").join("feature"))
            .expect("create ts5 types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["future/index.d.ts"],
                        "feature/*": ["future/feature/*.d.ts"]
                    },
                    "5.* || ^4.8": {
                        "dist/index.d.ts": ["ts5/index.d.ts"],
                        "feature/*": ["ts5/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface Props { fallbackRoot: string }",
        )
        .expect("write fallback root types");
        std::fs::write(
            versioned_pkg.join("future").join("index.d.ts"),
            "export interface Props { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write future root types");
        std::fs::write(
            versioned_pkg
                .join("future")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { futureFeature: string }",
        )
        .expect("write future feature types");
        std::fs::write(
            versioned_pkg.join("future").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write future model types");
        std::fs::write(
            versioned_pkg.join("ts5").join("index.d.ts"),
            "export interface Props { root: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts5 root types");
        std::fs::write(
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            "export type FeatureProps = { feature?: number }",
        )
        .expect("write ts5 feature types");
        std::fs::write(
            versioned_pkg.join("ts5").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts5 model types");

        let ambient_pkg = node_modules
            .join("@types")
            .join("vuec-bridge-typesversions-ambient");
        std::fs::create_dir_all(ambient_pkg.join("ts5")).expect("create @types versioned");
        std::fs::write(
            ambient_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "~5.0": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write @types package manifest");
        std::fs::write(
            ambient_pkg.join("index.d.ts"),
            "export type AmbientProps = { ambientFallback: number }",
        )
        .expect("write fallback @types");
        std::fs::write(
            ambient_pkg.join("ts5").join("index.d.ts"),
            "export type AmbientProps = { ambient: boolean }",
        )
        .expect("write ts5 @types");

        let type_root_pkg = dir.join("typings").join("versioned-global");
        std::fs::create_dir_all(type_root_pkg.join("ts5")).expect("create type root package");
        std::fs::write(
            type_root_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    "5.0 - 5.9": {
                        "index.d.ts": ["ts5/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write type root package manifest");
        std::fs::write(
            type_root_pkg.join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRootFallback: number }",
        )
        .expect("write fallback type root global");
        std::fs::write(
            type_root_pkg.join("ts5").join("index.d.ts"),
            "declare interface TypeRootGlobalProps { typeRoot: string }",
        )
        .expect("write ts5 type root global");

        let ordered_pkg = node_modules.join("vuec-bridge-typesversions-ordered");
        std::fs::create_dir_all(ordered_pkg.join("first")).expect("create first ordered types");
        std::fs::create_dir_all(ordered_pkg.join("second")).expect("create second ordered types");
        std::fs::create_dir_all(ordered_pkg.join("fallback"))
            .expect("create fallback ordered types");
        std::fs::write(
            ordered_pkg.join("package.json"),
            r#"{
                "types": "index.d.ts",
                "typesVersions": {
                    ">=4.8": {
                        "index.d.ts": ["first/index.d.ts"]
                    },
                    ">=5.0": {
                        "index.d.ts": ["second/index.d.ts"]
                    },
                    "*": {
                        "index.d.ts": ["fallback/index.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write ordered package manifest");
        std::fs::write(
            ordered_pkg.join("index.d.ts"),
            "export type OrderedProps = { orderedFallbackRoot: boolean }",
        )
        .expect("write ordered root fallback");
        std::fs::write(
            ordered_pkg.join("first").join("index.d.ts"),
            "export type OrderedProps = { orderedFirst: string }",
        )
        .expect("write first ordered types");
        std::fs::write(
            ordered_pkg.join("second").join("index.d.ts"),
            "export type OrderedProps = { orderedSecond: number }",
        )
        .expect("write second ordered types");
        std::fs::write(
            ordered_pkg.join("fallback").join("index.d.ts"),
            "export type OrderedProps = { orderedFallback: boolean }",
        )
        .expect("write fallback ordered types");

        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "types": ["versioned-global"],
                    "typeRoots": ["./typings"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from 'vuec-bridge-typesversions'\n",
                    "import type { FeatureProps } from 'vuec-bridge-typesversions/feature/item'\n",
                    "import type { AmbientProps } from 'vuec-bridge-typesversions-ambient'\n",
                    "import type { OrderedProps } from 'vuec-bridge-typesversions-ordered'\n",
                    "defineProps<Props & FeatureProps & AmbientProps & TypeRootGlobalProps & OrderedProps>()\n",
                    "defineModel<import('vuec-bridge-typesversions').ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("feature: { type: Number, required: false }"));
        assert!(content.contains("ambient: { type: Boolean, required: true }"));
        assert!(content.contains("typeRoot: { type: String, required: true }"));
        assert!(content.contains("orderedFirst: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!content.contains("fallbackRoot"));
        assert!(!content.contains("futureRoot"));
        assert!(!content.contains("futureFeature"));
        assert!(!content.contains("ambientFallback"));
        assert!(!content.contains("typeRootFallback"));
        assert!(!content.contains("orderedSecond"));
        assert!(!content.contains("orderedFallback"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts5").join("index.d.ts"),
            versioned_pkg.join("ts5").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts5").join("model.d.ts"),
            ambient_pkg.join("ts5").join("index.d.ts"),
            type_root_pkg.join("ts5").join("index.d.ts"),
            ordered_pkg.join("first").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_package_types_versions_from_project_typescript() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-types-versions-project-ts-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let node_modules = dir.join("node_modules");
        let typescript_pkg = node_modules.join("typescript");
        std::fs::create_dir_all(&typescript_pkg).expect("create typescript package");
        std::fs::write(
            typescript_pkg.join("package.json"),
            r#"{"version":"5.2.0"}"#,
        )
        .expect("write typescript manifest");

        let versioned_pkg = node_modules.join("vuec-bridge-typesversions-project-ts");
        std::fs::create_dir_all(versioned_pkg.join("dist")).expect("create dist types");
        std::fs::create_dir_all(versioned_pkg.join("ts52").join("feature"))
            .expect("create ts52 types");
        std::fs::create_dir_all(versioned_pkg.join("ts50").join("feature"))
            .expect("create ts50 types");
        std::fs::create_dir_all(versioned_pkg.join("legacy").join("feature"))
            .expect("create legacy types");
        std::fs::write(
            versioned_pkg.join("package.json"),
            r#"{
                "types": "dist/index.d.ts",
                "typesVersions": {
                    ">=5.1": {
                        "dist/index.d.ts": ["ts52/index.d.ts"],
                        "feature/*": ["ts52/feature/*.d.ts"]
                    },
                    ">=5.0": {
                        "dist/index.d.ts": ["ts50/index.d.ts"],
                        "feature/*": ["ts50/feature/*.d.ts"]
                    },
                    "*": {
                        "dist/index.d.ts": ["legacy/index.d.ts"],
                        "feature/*": ["legacy/feature/*.d.ts"]
                    }
                }
            }"#,
        )
        .expect("write versioned package manifest");
        std::fs::write(
            versioned_pkg.join("dist").join("index.d.ts"),
            "export interface Props { fallbackRoot: string }",
        )
        .expect("write dist fallback types");
        std::fs::write(
            versioned_pkg.join("legacy").join("index.d.ts"),
            "export interface Props { legacyRoot: string }",
        )
        .expect("write legacy root types");
        std::fs::write(
            versioned_pkg
                .join("legacy")
                .join("feature")
                .join("item.d.ts"),
            "export type FeatureProps = { legacyFeature: string }",
        )
        .expect("write legacy feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("index.d.ts"),
            "export interface Props { baselineRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts50 root types");
        std::fs::write(
            versioned_pkg.join("ts50").join("feature").join("item.d.ts"),
            "export type FeatureProps = { baselineFeature: boolean }",
        )
        .expect("write ts50 feature types");
        std::fs::write(
            versioned_pkg.join("ts50").join("model.d.ts"),
            "export type ModelValue = boolean | string",
        )
        .expect("write ts50 model types");
        std::fs::write(
            versioned_pkg.join("ts52").join("index.d.ts"),
            "export interface Props { futureRoot: string }\nexport type ModelValue = import('./model').ModelValue",
        )
        .expect("write ts52 root types");
        std::fs::write(
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            "export type FeatureProps = { futureFeature?: number }",
        )
        .expect("write ts52 feature types");
        std::fs::write(
            versioned_pkg.join("ts52").join("model.d.ts"),
            "export type ModelValue = number",
        )
        .expect("write ts52 model types");

        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { Props } from 'vuec-bridge-typesversions-project-ts'\n",
                    "import type { FeatureProps } from 'vuec-bridge-typesversions-project-ts/feature/item'\n",
                    "defineProps<Props & FeatureProps>()\n",
                    "defineModel<import('vuec-bridge-typesversions-project-ts').ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("futureRoot: { type: String, required: true }"));
        assert!(content.contains("futureFeature: { type: Number, required: false }"));
        assert!(content.contains("\"modelValue\": { type: Number },"));
        assert!(!content.contains("baselineRoot"));
        assert!(!content.contains("baselineFeature"));
        assert!(!content.contains("legacyRoot"));
        assert!(!content.contains("legacyFeature"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            versioned_pkg.join("ts52").join("index.d.ts"),
            versioned_pkg.join("ts52").join("feature").join("item.d.ts"),
            versioned_pkg.join("ts52").join("model.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_tsconfig_path_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-path-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("src").join("views")).expect("create views");
        std::fs::create_dir_all(dir.join("tsconfigs")).expect("create tsconfigs");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "files": [],
                "references": [{ "path": "./tsconfig.app.json" }],
                "compilerOptions": {
                    "paths": {
                        "bar": ["./pp.ts"]
                    }
                }
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("tsconfig.app.json"),
            r#"{
                "extends": ["./tsconfigs/base.json"]
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.join("tsconfigs").join("base.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "@/*": ["${configDir}/src/*"]
                    }
                },
                "include": ["${configDir}/src/**/*.ts", "${configDir}/src/**/*.vue"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(dir.join("pp.ts"), "export type PathProps = { bar: string }")
            .expect("write path type");
        std::fs::write(
            dir.join("src").join("types.ts"),
            "export type BaseProps = { foo?: string; count: number }",
        )
        .expect("write aliased type");
        std::fs::write(
            dir.join("src").join("views").join("Aliased.vue"),
            "<script lang=\"ts\">export type VueProps = { fromVue: string }</script>",
        )
        .expect("write aliased vue");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { PathProps } from 'bar'\n",
                    "import type { BaseProps } from '@/types.ts'\n",
                    "import type { VueProps } from '@/views/Aliased.vue'\n",
                    "defineProps<PathProps & BaseProps & VueProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("bar: { type: String, required: true }"));
        assert!(content.contains("foo: { type: String, required: false }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert!(content.contains("fromVue: { type: String, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("pp.ts"),
            dir.join("src").join("types.ts"),
            dir.join("src").join("views").join("Aliased.vue"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_tsconfig_jsonc_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-jsonc-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("src").join("base")).expect("create base");
        std::fs::create_dir_all(dir.join("config")).expect("create config");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                // Root alias survives comments and trailing commas.
                "compilerOptions": {
                    "paths": {
                        "root-alias": ["./root.ts",],
                    },
                },
                "references": [
                    { "path": "./tsconfig.app.json", },
                ],
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("tsconfig.app.json"),
            r#"{
                "extends": [
                    "./config/base.json", // inherited aliases
                ],
                "compilerOptions": {
                    "paths": {
                        "app-alias": ["./app.ts",],
                    },
                },
            }"#,
        )
        .expect("write app tsconfig");
        std::fs::write(
            dir.join("config").join("base.json"),
            r#"{
                /* ${configDir} resolves from the referencing config directory. */
                "compilerOptions": {
                    "paths": {
                        "@base/*": ["${configDir}/src/base/*",],
                    },
                },
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.join("app.ts"),
            "export type AppProps = { app?: number }",
        )
        .expect("write app type");
        std::fs::write(
            dir.join("src").join("base").join("types.ts"),
            "export type BaseProps = { base: boolean }",
        )
        .expect("write base type");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { RootProps } from 'root-alias'\n",
                    "import type { AppProps } from 'app-alias'\n",
                    "import type { BaseProps } from '@base/types'\n",
                    "defineProps<RootProps & AppProps & BaseProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("app: { type: Number, required: false }"));
        assert!(content.contains("base: { type: Boolean, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("root.ts"),
            dir.join("app.ts"),
            dir.join("src").join("base").join("types.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_package_tsconfig_extends_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-package-tsconfig-extends-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");

        let scoped_config_pkg = dir.join("node_modules").join("@vuec").join("tsconfig");
        std::fs::create_dir_all(&scoped_config_pkg).expect("create scoped config package");
        std::fs::write(
            scoped_config_pkg.join("package.json"),
            r#"{"tsconfig":"base.json"}"#,
        )
        .expect("write scoped config package manifest");
        std::fs::write(
            scoped_config_pkg.join("base.json"),
            r#"{
                // Package config entries may be JSONC.
                "compilerOptions": {
                    "paths": {
                        "pkg-root": ["${configDir}/root.ts",],
                    },
                },
            }"#,
        )
        .expect("write scoped package config");

        let preset_pkg = dir.join("node_modules").join("vuec-tsconfig-preset");
        std::fs::create_dir_all(&preset_pkg).expect("create preset package");
        std::fs::write(
            preset_pkg.join("shared.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "pkg-shared": ["${configDir}/shared.ts"]
                    }
                }
            }"#,
        )
        .expect("write preset subpath config");

        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "extends": ["@vuec/tsconfig", "vuec-tsconfig-preset/shared"],
                "compilerOptions": {
                    "paths": {
                        "local-alias": ["./local.ts"]
                    }
                }
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("root.ts"),
            "export type RootProps = { root: string }",
        )
        .expect("write root type");
        std::fs::write(
            dir.join("shared.ts"),
            "export type SharedProps = { shared?: number }",
        )
        .expect("write shared type");
        std::fs::write(
            dir.join("local.ts"),
            "export type LocalProps = { local: boolean }",
        )
        .expect("write local type");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import type { RootProps } from 'pkg-root'\n",
                    "import type { SharedProps } from 'pkg-shared'\n",
                    "import type { LocalProps } from 'local-alias'\n",
                    "defineProps<RootProps & SharedProps & LocalProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("shared: { type: Number, required: false }"));
        assert!(content.contains("local: { type: Boolean, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("root.ts"),
            dir.join("shared.ts"),
            dir.join("local.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_global_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-global-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let global = dir.join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare interface GlobalProps { msg: string }\n",
                "declare type GlobalModel = boolean | string"
            ),
        )
        .expect("write global types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<GlobalProps>()\n",
                    "defineModel<GlobalModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy(),
                "options": {
                    "globalTypeFiles": [global.to_string_lossy()]
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let expected_dep = global.to_string_lossy().replace('\\', "/");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("msg: { type: String, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(compiled["deps"], json!([expected_dep]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_discovers_tsconfig_global_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-global-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("types").join("nested")).expect("create types");
        std::fs::create_dir_all(dir.join("config")).expect("create config");
        std::fs::create_dir_all(dir.join("project")).expect("create project");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "files": ["./types/root.d.ts"],
                "include": ["./types/**/*.ts", "./src/**/*.vue"],
                "extends": "./config/base.json",
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("config").join("base.json"),
            r#"{
                "files": ["${configDir}/types/base.d.ts"]
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(
            dir.join("project").join("tsconfig.json"),
            r#"{
                "files": ["../types/ref.d.ts"]
            }"#,
        )
        .expect("write referenced tsconfig");
        std::fs::write(
            dir.join("types").join("root.d.ts"),
            "declare interface RootGlobalProps { root: string }",
        )
        .expect("write root global");
        std::fs::write(
            dir.join("types").join("nested").join("included.d.ts"),
            "declare interface IncludedGlobalProps { included?: number }",
        )
        .expect("write included global");
        std::fs::write(
            dir.join("types").join("base.d.ts"),
            "declare interface BaseGlobalProps { base: boolean }",
        )
        .expect("write base global");
        std::fs::write(
            dir.join("types").join("ref.d.ts"),
            "declare type RefGlobalModel = boolean | string",
        )
        .expect("write referenced global");
        std::fs::write(
            dir.join("src").join("ignored.d.ts"),
            "declare interface IgnoredByVueInclude { ignored: string }",
        )
        .expect("write ignored global");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<RootGlobalProps & IncludedGlobalProps & BaseGlobalProps>()\n",
                    "defineModel<RefGlobalModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("root: { type: String, required: true }"));
        assert!(content.contains("included: { type: Number, required: false }"));
        assert!(content.contains("base: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(!content.contains("ignored: { type: String, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("types").join("root.d.ts"),
            dir.join("types").join("nested").join("included.d.ts"),
            dir.join("types").join("base.d.ts"),
            dir.join("types").join("ref.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!deps.iter().any(|dep| dep.contains("ignored")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_discovers_tsconfig_types_type_roots_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-tsconfig-types-type-roots-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("typings").join("chosen")).expect("create chosen");
        std::fs::create_dir_all(dir.join("typings").join("@scope").join("tool"))
            .expect("create scoped");
        std::fs::create_dir_all(dir.join("typings").join("ignored")).expect("create ignored");
        std::fs::create_dir_all(dir.join("base-types").join("base-root")).expect("create base");
        std::fs::create_dir_all(dir.join("node_modules").join("@types").join("defaulted"))
            .expect("create default @types");
        std::fs::create_dir_all(dir.join("config")).expect("create config");
        std::fs::create_dir_all(dir.join("project")).expect("create project");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "extends": "./config/base.json",
                "compilerOptions": {
                    "types": ["chosen", "@scope/tool"],
                    "typeRoots": ["./typings"]
                },
                "references": [{ "path": "./project" }]
            }"#,
        )
        .expect("write root tsconfig");
        std::fs::write(
            dir.join("config").join("base.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["${configDir}/base-types"]
                }
            }"#,
        )
        .expect("write base tsconfig");
        std::fs::write(dir.join("project").join("tsconfig.json"), "{}")
            .expect("write referenced tsconfig");
        std::fs::write(
            dir.join("typings").join("chosen").join("index.d.ts"),
            "declare interface ChosenGlobalProps { chosen: string }",
        )
        .expect("write chosen global");
        std::fs::write(
            dir.join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            "declare type ScopedGlobalModel = number | boolean",
        )
        .expect("write scoped global");
        std::fs::write(
            dir.join("typings").join("ignored").join("index.d.ts"),
            "declare interface IgnoredTypeRootGlobalProps { ignored: string }",
        )
        .expect("write ignored global");
        std::fs::write(
            dir.join("base-types").join("base-root").join("index.d.ts"),
            "declare interface BaseRootGlobalProps { baseRoot?: number }",
        )
        .expect("write base root global");
        std::fs::write(
            dir.join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<ChosenGlobalProps & BaseRootGlobalProps & DefaultTypesGlobalProps>()\n",
                    "defineModel<ScopedGlobalModel>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("chosen: { type: String, required: true }"));
        assert!(content.contains("baseRoot: { type: Number, required: false }"));
        assert!(content.contains("defaulted: { type: Boolean, required: true }"));
        assert!(content.contains("\"modelValue\": { type: [Number, Boolean] },"));
        assert!(!content.contains("ignored: { type: String"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            dir.join("base-types").join("base-root").join("index.d.ts"),
            dir.join("typings").join("chosen").join("index.d.ts"),
            dir.join("typings")
                .join("@scope")
                .join("tool")
                .join("index.d.ts"),
            dir.join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);
        assert!(!deps.iter().any(|dep| dep.contains("ignored")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_respects_empty_configured_tsconfig_type_roots() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-empty-tsconfig-type-roots-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src").join("components")).expect("create components");
        std::fs::create_dir_all(dir.join("node_modules").join("@types").join("defaulted"))
            .expect("create default @types");
        std::fs::write(
            dir.join("node_modules")
                .join("@types")
                .join("defaulted")
                .join("index.d.ts"),
            "declare interface DefaultTypesGlobalProps { defaulted: boolean }",
        )
        .expect("write default @types global");
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{
                "compilerOptions": {
                    "typeRoots": ["./missing"]
                }
            }"#,
        )
        .expect("write tsconfig");

        let filename = dir.join("src").join("components").join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<DefaultTypesGlobalProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let errors = compiled["errors"].as_array().unwrap();
        assert!(errors
            .iter()
            .any(|error| error.as_str().is_some_and(|message| {
                message.contains("Unresolvable type reference or unsupported built-in utility type")
            })));
        assert!(compiled["deps"].as_array().unwrap().is_empty());
        assert!(!compiled["content"]
            .as_str()
            .unwrap_or_default()
            .contains("defaulted: { type: Boolean"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_resolves_global_type_re_exports_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-global-re-export-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("node_modules").join("pkg").join("dist"))
            .expect("create package");
        std::fs::write(dir.join("base.ts"), "export interface Base { age: number }")
            .expect("write base type");
        std::fs::write(dir.join("types.ts"), "export type Name = string")
            .expect("write helper type");
        std::fs::write(
            dir.join("foo.ts"),
            concat!(
                "import type { Base } from './base'\n",
                "import type { Name } from './types'\n",
                "export interface Foo extends Base { name: Name }"
            ),
        )
        .expect("write foo type");
        std::fs::write(dir.join("bar.ts"), "export interface Bar { bar: boolean }")
            .expect("write bar type");
        std::fs::write(dir.join("baz.ts"), "export interface Baz { baz: string }")
            .expect("write baz type");
        let package_dir = dir.join("node_modules").join("pkg");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"types":"dist/index.d.ts"}"#,
        )
        .expect("write package manifest");
        std::fs::write(
            package_dir.join("dist").join("index.d.ts"),
            "export interface PackageType { value: string }",
        )
        .expect("write package types");
        let global = dir.join("global.d.ts");
        std::fs::write(
            &global,
            concat!(
                "declare global {\n",
                "  export type { Foo } from './foo'\n",
                "  export { Bar } from './bar'\n",
                "  export * from './baz'\n",
                "  export type { PackageType } from './node_modules/pkg'\n",
                "}\n",
                "export {}\n"
            ),
        )
        .expect("write global re-exports");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<Foo & Bar & Baz & PackageType>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy(),
                "options": {
                    "globalTypeFiles": [global.to_string_lossy()]
                }
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("age: { type: Number, required: true }"));
        assert!(content.contains("name: { type: String, required: true }"));
        assert!(content.contains("bar: { type: Boolean, required: true }"));
        assert!(content.contains("baz: { type: String, required: true }"));
        assert!(content.contains("value: { type: String, required: true }"));

        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            global,
            dir.join("foo.ts"),
            dir.join("base.ts"),
            dir.join("types.ts"),
            dir.join("bar.ts"),
            dir.join("baz.ts"),
            package_dir.join("dist").join("index.d.ts"),
        ]
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_dynamic_import_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-dynamic-import-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("foo.ts"),
            "export type Props = { foo: string, count: import('./bar').Count }",
        )
        .expect("write props");
        std::fs::write(dir.join("bar.ts"), "export type Count = number").expect("write bar");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "defineProps<import('./foo').Props>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["foo.ts", "bar.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: true }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_namespace_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-namespace-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("types.ts"),
            "export type Props = { foo: string }\nexport type Events = { (e: 'save'): void }\nexport type ModelValue = boolean | string",
        )
        .expect("write namespace types");
        std::fs::write(
            dir.join("leaf.ts"),
            "export namespace Nested { export type ExtraProps = { count?: number } }",
        )
        .expect("write leaf types");
        std::fs::write(
            dir.join("dynamic.ts"),
            "export namespace Types { export type Props = { bar: number } }",
        )
        .expect("write dynamic types");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import * as Types from './types'\n",
                    "import * as Leaf from './leaf'\n",
                    "const props = defineProps<Types.Props & Leaf.Nested.ExtraProps & import('./dynamic').Types.Props>()\n",
                    "const emit = defineEmits<Types.Events>()\n",
                    "const model = defineModel<Types.ModelValue>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["types.ts", "leaf.ts", "dynamic.ts"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: false }"));
        assert!(content.contains("bar: { type: Number, required: true }"));
        assert!(content
            .contains("emits: /*@__PURE__*/_mergeModels([\"save\"], [\"update:modelValue\"]),"));
        assert!(content.contains("\"modelValue\": { type: [Boolean, String] },"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_returns_vue_type_deps() {
        let dir = std::env::temp_dir().join(format!(
            "vuec-node-bridge-vue-type-deps-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("foo.vue"),
            "<template><div /></template><script lang=\"ts\">export type Props = { foo: string }</script>",
        )
        .expect("write foo vue");
        std::fs::write(
            dir.join("bar.vue"),
            "<script setup lang=\"ts\">export type ExtraProps = { count?: number }</script>",
        )
        .expect("write bar vue");

        let filename = dir.join("Comp.vue");
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "import { Props } from './foo.vue'\n",
                    "import { ExtraProps } from './bar.vue'\n",
                    "defineProps<Props & ExtraProps>()",
                    "</script>"
                ),
                "filename": filename.to_string_lossy()
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let deps = compiled["deps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected = ["foo.vue", "bar.vue"]
            .into_iter()
            .map(|name| dir.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(content.contains("foo: { type: String, required: true }"));
        assert!(content.contains("count: { type: Number, required: false }"));
        assert_eq!(deps, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vue3_sfc_bridge_compile_script_splits_define_model_transformers() {
        let compiled = dispatch(
            "sfc.compileScript",
            json!({
                "source": concat!(
                    "<script setup lang=\"ts\">",
                    "const modelValue = defineModel({\n",
                    "  get(v) { return v - 1 },\n",
                    "  set: (v) => { return v + 1 },\n",
                    "  required: true\n",
                    "})\n",
                    "const count = defineModel<number>('count', {\n",
                    "  get(v) { return v },\n",
                    "  required: true,\n",
                    "})",
                    "</script>"
                ),
                "filename": "FooBar.vue"
            }),
        )
        .expect("vue3 compileScript");

        let content = compiled["content"].as_str().unwrap_or_default();
        let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(compiled["errors"].as_array().unwrap().is_empty());
        assert!(compact.contains("\"modelValue\": { required: true },"));
        assert!(compact.contains("\"count\": { type: Number, ...{ required: true, } },"));
        assert!(compact.contains("const modelValue = _useModel(__props, \"modelValue\", { get(v) { return v - 1 }, set: (v) => { return v + 1 }, })"));
        assert!(compact.contains(
            "const count = _useModel<number>(__props, 'count', { get(v) { return v }, })"
        ));
        assert_eq!(compiled["bindings"]["modelValue"], json!("setup-ref"));
        assert_eq!(compiled["bindings"]["count"], json!("setup-ref"));
    }

    #[test]
    fn vue3_sfc_bridge_parse_projects_public_descriptor_shape() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template><div>{{ msg }}</div></template>",
                    "<script setup lang=\"ts\">const msg: string = 'hi'</script>",
                    "<style scoped>.a{color:v-bind(color)}</style>",
                    "<i18n lang=\"json\">{\"en\":\"hi\"}</i18n>"
                ),
                "filename": "Comp.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["type"], json!("template"));
        assert_eq!(
            descriptor["template"]["loc"]["source"],
            json!("<div>{{ msg }}</div>")
        );
        assert_eq!(
            descriptor["template"]["ast"]["source"],
            descriptor["source"]
        );
        assert_eq!(
            descriptor["template"]["ast"]["children"][0]["tag"],
            json!("div")
        );
        assert_eq!(descriptor["scriptSetup"]["setup"], json!(true));
        assert_eq!(descriptor["scriptSetup"]["lang"], json!("ts"));
        assert_eq!(descriptor["styles"][0]["scoped"], json!(true));
        assert_eq!(descriptor["cssVars"], json!(["color"]));
        assert_eq!(descriptor["customBlocks"][0]["type"], json!("i18n"));
        assert!(descriptor.get("script_setup").is_none());
        assert_eq!(parsed["errors"], json!([]));
    }

    #[test]
    fn vue3_sfc_bridge_parse_projects_plain_template_lang_as_text_ast() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template lang=\"pug\">p(v-if=\"1 < 2\") test <div/></template>",
                "filename": "Pug.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let ast = &parsed["descriptor"]["template"]["ast"];
        assert_eq!(parsed["errors"], json!([]));
        assert_eq!(ast["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            ast["children"][0]["content"],
            json!("p(v-if=\"1 < 2\") test <div/>")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_uses_dom_void_tags_and_template_options() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template><input><hello/></template><foo> <-& </foo>",
                "filename": "TemplateOptions.vue",
                "options": {
                    "sourceMap": false,
                    "templateParseOptions": {
                        "__vuecCustomElements": ["hello"]
                    }
                }
            }),
        )
        .expect("vue3 sfc parse");

        let template_children = parsed["descriptor"]["template"]["ast"]["children"]
            .as_array()
            .unwrap();
        assert_eq!(parsed["errors"], json!([]));
        assert_eq!(template_children[0]["tag"], json!("input"));
        assert_eq!(template_children[1]["tag"], json!("hello"));
        assert_eq!(template_children[1]["tagType"], json!(0));
        assert_eq!(
            parsed["descriptor"]["customBlocks"][0]["content"],
            json!(" <-& ")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_returns_descriptor_validation_errors() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template>a</template>",
                    "<template>b</template>",
                    "<script src=\"x\"></script>",
                    "<script setup>ok</script>"
                ),
                "filename": "Dup.vue"
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["content"], json!("a"));
        assert!(descriptor["script"].is_null());
        assert_eq!(descriptor["scriptSetup"]["content"], json!("ok"));
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("Single file component can contain only one <template> element")
        );
        assert_eq!(
            parsed["errors"][0]["loc"]["source"],
            json!("<template>b</template>")
        );
        assert_eq!(
            parsed["errors"][1]["message"],
            json!("<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together.")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_preserves_src_presence_and_functional_template_error() {
        let src_parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template src></template><script src></script><style src></style>",
                "filename": "BoolSrc.vue"
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &src_parsed["descriptor"];
        assert_eq!(descriptor["template"]["attrs"]["src"], json!(true));
        assert!(descriptor["template"].get("src").is_none());
        assert!(descriptor["template"].get("map").is_none());
        assert!(descriptor["template"].get("ast").is_none());
        assert_eq!(descriptor["script"]["attrs"]["src"], json!(true));
        assert_eq!(descriptor["styles"][0]["attrs"]["src"], json!(true));
        assert_eq!(src_parsed["errors"], json!([]));

        let functional = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template functional="x"><div/></template>"#,
                "filename": "Functional.vue"
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(
            functional["errors"][0]["message"],
            json!("<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.")
        );
        assert_eq!(
            functional["errors"][0]["loc"]["source"],
            json!("functional=\"x\"")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_decodes_attrs_and_duplicate_attr_errors() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template a="1" a="&amp;">x</template><style module="m&amp;n" setup>.a{}</style>"#,
                "filename": "Attrs.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["attrs"]["a"], json!("&"));
        assert_eq!(descriptor["styles"][0]["module"], json!("m&n"));
        assert!(descriptor["styles"][0].get("setup").is_none());
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("Duplicate attribute.")
        );
        assert_eq!(parsed["errors"][0]["loc"]["start"]["offset"], json!(16));
    }

    #[test]
    fn vue3_sfc_bridge_parse_reports_syntax_errors_from_descriptor_scan() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<?xml?><template><?x?><div/></template><docs><?keep?></docs>"#,
                "filename": "Syntax.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        assert_eq!(
            parsed["descriptor"]["template"]["content"],
            json!("<?x?><div/>")
        );
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("'<?' is allowed only in XML context.")
        );
        assert_eq!(parsed["errors"][0]["loc"]["start"]["offset"], json!(1));
        assert_eq!(parsed["errors"][1]["loc"]["start"]["offset"], json!(18));

        let unclosed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template><div><span>",
                "filename": "Unclosed.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(unclosed["descriptor"]["template"]["content"], json!(""));
        assert_eq!(
            unclosed["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["loc"]["start"]["offset"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![15, 10, 0]
        );

        let malformed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template><div id id></div></template><script>const s = "</script>";</script>"#,
                "filename": "Malformed.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(
            malformed["descriptor"]["script"]["content"],
            json!("const s = \"")
        );
        assert_eq!(
            malformed["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["message"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Duplicate attribute.", "Invalid end tag."]
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_applies_padding_and_ignore_empty_options() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template lang=\"pug\">\n  div\n</template>\n",
                    "<script>\nconst a = 1\n</script>\n",
                    "<style> </style>"
                ),
                "filename": "Pad.vue",
                "options": {
                    "pad": "line",
                    "ignoreEmpty": false,
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["content"], json!("\ndiv\n"));
        assert_eq!(
            descriptor["script"]["content"],
            json!("//\n//\n//\n\nconst a = 1\n")
        );
        assert_eq!(descriptor["styles"][0]["content"], json!("\n\n\n\n\n\n "));
    }

    #[test]
    fn vue27_bridge_compile_script_passes_css_var_options() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>const a = 1</script><style>div{ color: v-bind(color); }</style>",
                "filename": "test.vue",
                "options": {
                    "id": "xxxxxxxx",
                    "isProd": true
                }
            }),
        )
        .expect("vue27 script");

        let content = compiled["content"].as_str().unwrap_or("");
        assert!(content.contains("\"4003f1a6\": (_vm.color)"));
        assert!(content.contains("export default __default__"));
    }

    #[test]
    fn vue27_bridge_compile_script_preserves_script_ast_and_internal_binding_flag() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>export default { props: ['foo'] }</script>",
                "filename": "test.vue",
                "options": {}
            }),
        )
        .expect("vue27 script");

        let script_ast = compiled["scriptAst"].as_array().expect("scriptAst array");
        assert_eq!(script_ast.len(), 1);
        assert_eq!(script_ast[0]["type"], json!("ExportDefaultDeclaration"));
        assert_eq!(
            script_ast[0]["source"],
            json!("export default { props: ['foo'] }")
        );
        assert_eq!(script_ast[0]["loc"]["start"]["offset"], json!(0));
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["__isScriptSetup"], json!("false"));
    }

    #[test]
    fn vue27_bridge_compile_script_honors_internal_script_ast_mode() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>export default { props: ['foo'] }</script>",
                "filename": "test.vue",
                "options": {
                    "__vuecScriptAstMode": "none"
                }
            }),
        )
        .expect("vue27 script");

        assert!(compiled.get("scriptAst").is_none());
        assert!(compiled.get("scriptSetupAst").is_none());
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
    }

    #[test]
    fn vue3_dom_bridge_uses_dom_builtin_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({ "source": "<transition/><transition-group/>", "options": {} }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["tagType"], json!(1));
        assert_eq!(parsed["children"][1]["tagType"], json!(1));

        let compiled = dispatch(
            "vue3.dom.compile",
            json!({ "source": "<transition><div/><div/></transition>", "options": {} }),
        )
        .expect("dom compile");

        assert!(compiled["code"]
            .as_str()
            .unwrap_or("")
            .contains("_Transition"));
        assert!(compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(
                |diagnostic| diagnostic.get("message").and_then(Value::as_str)
                    == Some("<Transition> expects exactly one child element or component.")
            ));
    }

    #[test]
    fn vue3_dom_bridge_projects_compile_diagnostic_objects() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<div :bar="a[" v-model="baz"/>"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("dom compile");

        let diagnostics = compiled["diagnostics"].as_array().expect("diagnostics");
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0]["code"], json!(46));
        assert!(diagnostics[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Error parsing JavaScript expression: Unexpected token"));
        assert_eq!(diagnostics[0]["loc"]["start"]["offset"], json!(13));
        assert_eq!(diagnostics[1]["code"], json!(58));
        assert_eq!(diagnostics[1]["loc"]["source"], json!("v-model=\"baz\""));
    }

    #[test]
    fn vue3_dom_bridge_projects_template_expression_public_ast() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": concat!(
                    r#"<FooBar #[foo.slotName] :class="[cond ? '' : bar(), 'default']">"#,
                    r#"{{ `${VAR}VAR2${VAR3}` }}{{ Foo.Bar.Baz }}"#,
                    r#"</FooBar>"#
                ),
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("dom parse");

        let node = &parsed["children"][0];
        let dynamic_arg = &node["props"][0]["arg"]["ast"];
        assert_eq!(dynamic_arg["type"], json!("MemberExpression"));
        assert_eq!(dynamic_arg["object"]["name"], json!("foo"));
        assert_eq!(dynamic_arg["property"]["name"], json!("slotName"));

        let class_exp = &node["props"][1]["exp"]["ast"];
        assert_eq!(class_exp["type"], json!("ArrayExpression"));
        assert_eq!(
            class_exp["elements"][0]["type"],
            json!("ConditionalExpression")
        );
        assert_eq!(class_exp["elements"][0]["test"]["name"], json!("cond"));
        assert_eq!(
            class_exp["elements"][0]["alternate"]["callee"]["name"],
            json!("bar")
        );

        let template_literal = &node["children"][0]["content"]["ast"];
        assert_eq!(template_literal["type"], json!("TemplateLiteral"));
        assert_eq!(template_literal["expressions"][0]["name"], json!("VAR"));
        assert_eq!(template_literal["expressions"][1]["name"], json!("VAR3"));

        let member = &node["children"][1]["content"]["ast"];
        assert_eq!(member["type"], json!("MemberExpression"));
        assert_eq!(member["object"]["object"]["name"], json!("Foo"));
        assert_eq!(member["object"]["property"]["name"], json!("Bar"));
        assert_eq!(member["property"]["name"], json!("Baz"));
    }

    #[test]
    fn vue3_dom_bridge_compile_ast_slices_sfc_template_children() {
        let source =
            "<template><div>{{ msg }}</div></template><script>boom()</script><style>.x{}</style>";
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": source,
                "ast": {
                    "type": 0,
                    "source": source,
                    "children": [{
                        "type": 1,
                        "tag": "div",
                        "loc": {
                            "start": { "offset": 10 },
                            "end": { "offset": 30 },
                            "source": "<div>{{ msg }}</div>"
                        }
                    }]
                },
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true,
                    "__vuecSourceMapSource": source,
                    "__vuecSourceMapBaseOffset": 0
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_ctx.msg"));
        assert!(!compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|diagnostic| diagnostic
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("side effect")));
        assert_eq!(compiled["map"]["sourcesContent"][0], source);
        assert!(compiled["map"]["mappings"].as_str().unwrap_or("").len() > 4);
    }

    #[test]
    fn vue3_ssr_bridge_compile_ast_slices_sfc_template_children() {
        let source =
            "<template><div>{{ msg }}</div></template><script>boom()</script><style>.x{}</style>";
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": source,
                "ast": {
                    "type": 0,
                    "source": source,
                    "children": [{
                        "type": 1,
                        "tag": "div",
                        "loc": {
                            "start": { "offset": 10 },
                            "end": { "offset": 30 },
                            "source": "<div>{{ msg }}</div>"
                        }
                    }]
                },
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true,
                    "__vuecSourceMapSource": source,
                    "__vuecSourceMapBaseOffset": 0
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(!code.contains("boom"));
        assert_eq!(compiled["map"]["sources"], json!(["anonymous.vue"]));
        assert_eq!(compiled["map"]["sourcesContent"][0], source);
        assert!(compiled["map"]["mappings"].as_str().unwrap_or("").len() > 4);
    }

    #[test]
    fn vue3_ssr_bridge_uses_public_compile_defaults() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": "<div>{{ msg }}</div>",
                "options": {
                    "prefixIdentifiers": false,
                    "cacheHandlers": true,
                    "hoistStatic": true,
                    "scopeId": "data-v-x"
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(!code.contains("with (_ctx)"));
        assert!(code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(code.contains("data-v-x"));
        assert!(!code.contains("_hoisted_"));
        assert!(!code.contains("_cache["));
    }

    #[test]
    fn vue3_ssr_bridge_ignores_scope_id_for_explicit_function_mode() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": "<div class=\"a\"></div>",
                "options": {
                    "mode": "function",
                    "scopeId": "data-v-ignored"
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(!code.contains("data-v-ignored"));
        assert!(code.contains("_ssrRenderAttrs(_mergeProps("));
    }

    #[test]
    fn vue3_dom_bridge_uses_dom_namespace_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({ "source": "<svg><rect/></svg><math><ms>1</ms></math>", "options": {} }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["ns"], json!(1));
        assert_eq!(parsed["children"][0]["children"][0]["ns"], json!(1));
        assert_eq!(parsed["children"][1]["ns"], json!(2));
        assert_eq!(parsed["children"][1]["children"][0]["ns"], json!(2));
    }

    #[test]
    fn vue3_dom_bridge_sfc_inner_loc_ends_at_closing_tag_start() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template>\n<div></div>\n</template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let template = &parsed["children"][0];
        assert_eq!(template["innerLoc"]["source"], json!("\n<div></div>\n"));
        assert_eq!(template["innerLoc"]["start"]["offset"], json!(10));
        assert_eq!(template["innerLoc"]["end"]["offset"], json!(23));
    }

    #[test]
    fn vue3_dom_bridge_sfc_inner_loc_offsets_are_utf16() {
        let source = r#"<script>import { "😏" as foo } from './foo'</script><script setup>import { "😏" as foo } from './foo'</script>"#;
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": source,
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let script = &parsed["children"][0];
        let script_setup = &parsed["children"][1];
        assert_eq!(
            script["innerLoc"]["source"],
            json!(r#"import { "😏" as foo } from './foo'"#)
        );
        assert_eq!(script["innerLoc"]["start"]["offset"], json!(8));
        assert_eq!(script["innerLoc"]["end"]["offset"], json!(43));
        assert_eq!(
            script_setup["innerLoc"]["source"],
            json!(r#"import { "😏" as foo } from './foo'"#)
        );
        assert_eq!(script_setup["innerLoc"]["start"]["offset"], json!(66));
        assert_eq!(script_setup["innerLoc"]["end"]["offset"], json!(101));
    }

    #[test]
    fn vue3_core_bridge_projects_public_utils() {
        let position = dispatch(
            "vue3.core.advancePositionWithClone",
            json!({
                "pos": { "offset": 0, "line": 1, "column": 1 },
                "source": "foo\nbar",
                "numberOfCharacters": 4,
            }),
        )
        .expect("position projection");
        assert_eq!(position, json!({ "offset": 4, "line": 2, "column": 1 }));

        let asset = dispatch(
            "vue3.core.toValidAssetId",
            json!({
                "name": "test-测试-1",
                "type": "component",
            }),
        )
        .expect("asset id projection");
        assert_eq!(asset["id"], json!("_component_test_2797935797_1"));
    }

    #[test]
    fn vue3_sfc_bridge_projects_template_utils_url_predicates() {
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isRelativeUrl",
                json!({ "url": "./logo.png" })
            )
            .expect("relative url"),
            json!(true)
        );
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isExternalUrl",
                json!({ "url": "https://vuejs.org/" })
            )
            .expect("external url"),
            json!(true)
        );
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isDataUrl",
                json!({ "url": "data:image/png;base64,i" })
            )
            .expect("data url"),
            json!(true)
        );
        assert_eq!(
            dispatch(
                "sfc.templateUtils.isRelativeUrl",
                json!({ "url": "/logo.png" })
            )
            .expect("absolute url"),
            json!(false)
        );
    }

    #[test]
    fn vue3_dom_bridge_sfc_plain_template_lang_keeps_raw_text() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template lang=\"pug\">p(v-if=\"1 < 2\") test <div/></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let template = &parsed["children"][0];
        assert_eq!(template["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            template["children"][0]["content"],
            json!("p(v-if=\"1 < 2\") test <div/>")
        );
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_parse_uses_dom_void_tag_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><input></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let input = &parsed["children"][0]["children"][0];
        assert_eq!(input["tag"], json!("input"));
        assert_eq!(input["children"].as_array().unwrap().len(), 0);
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_custom_blocks_are_raw_text() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><input></template><foo> <-& </foo>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let custom_block = &parsed["children"][1];
        assert_eq!(custom_block["tag"], json!("foo"));
        assert_eq!(custom_block["children"].as_array().unwrap().len(), 1);
        assert_eq!(custom_block["children"][0]["content"], json!(" <-& "));
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_parse_classifies_non_native_tags_as_components() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><hello/></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let hello = &parsed["children"][0]["children"][0];
        assert_eq!(hello["tag"], json!("hello"));
        assert_eq!(hello["tagType"], json!(1));

        let custom = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><hello/></template>",
                "options": {
                    "parseMode": "sfc",
                    "__vuecCustomElements": ["hello"]
                }
            }),
        )
        .expect("dom parse");
        assert_eq!(custom["children"][0]["children"][0]["tagType"], json!(0));
    }

    #[test]
    fn vue3_dom_bridge_allows_v_model_on_custom_elements() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<my-input v-model="value"/>"#,
                "options": {
                    "__vuecCustomElements": ["my-input"]
                }
            }),
        )
        .expect("dom compile");

        assert!(compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty());
        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("vModelText"));
        assert!(code.contains("_withDirectives"));
    }

    #[test]
    fn vue3_dom_bridge_respects_explicit_empty_dom_parser_predicates() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<input><hello/>",
                "options": {
                    "__vuecVoidTags": [],
                    "__vuecNativeTags": []
                }
            }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["children"][0]["tag"], json!("hello"));
        assert!(parsed["__vuecDiagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == json!(24)));
    }

    #[test]
    fn vue3_dom_bridge_parses_asset_url_options() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png"><img src="~bar.png">"#,
                "options": {
                    "transformAssetUrls": {
                        "base": "/foo"
                    }
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#"src: "/foo/bar.png""#));
        assert!(code.contains(r#"src: "~bar.png""#));

        let disabled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "transformAssetUrls": false
                }
            }),
        )
        .expect("dom compile");

        assert!(disabled["code"]
            .as_str()
            .unwrap_or("")
            .contains(r#"src: "./bar.png""#));
    }

    #[test]
    fn vue27_bridge_compile_template_transforms_asset_urls() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><img src="./logo.png" srcset="./logo.png 2x"><svg><use href="~@svg/file.svg#fragment"/></svg></div>"#,
                "options": {
                    "transformAssetUrls": {
                        "use": "href"
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#""src":require("./logo.png")"#));
        assert!(code.contains(r#""srcset":require("./logo.png") + " 2x""#));
        assert!(code.contains(r##""href":require("@svg/file.svg") + "#fragment""##));
    }

    #[test]
    fn vue27_bridge_compile_template_asset_options_support_base_and_absolute_urls() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><img src="./logo.png"><img src="/logo.png"><img src="@/logo.png"></div>"#,
                "options": {
                    "transformAssetUrls": true,
                    "transformAssetUrlsOptions": {
                        "base": "/base/",
                        "includeAbsolute": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#""src":"/base/logo.png""#));
        assert!(code.contains(r#""src":require("/logo.png")"#));
        assert!(code.contains(r#""src":require("@/logo.png")"#));
    }

    #[test]
    fn vue27_bridge_compile_template_preprocesses_pug_and_reports_missing_lang() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "body\n h1 Pug Examples\n div.container\n   p Cool Pug example!\n",
                "filename": "example.vue",
                "options": {
                    "preprocessLang": "pug"
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert!(compiled["errors"].as_array().unwrap().is_empty());
        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_c('body'"));
        assert!(code.contains("staticClass:\"container\""));

        let missing = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "",
                "filename": "example.vue",
                "options": {
                    "preprocessLang": "unknownLang"
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(missing["errors"].as_array().unwrap().len(), 1);
        assert_eq!(missing["tips"].as_array().unwrap().len(), 1);
        assert!(missing["errors"][0]
            .as_str()
            .unwrap_or("")
            .contains("unknownLang"));
        assert_eq!(
            missing["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
    }

    #[test]
    fn vue27_bridge_compile_template_returns_empty_render_on_vue2_errors() {
        let plain = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "<div></div><span></span><p></p>",
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": false
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(
            plain["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
        assert_eq!(plain["errors"].as_array().unwrap().len(), 1);
        assert_eq!(
            plain["errors"][0],
            json!("Component template should contain exactly one root element. If you are using v-if on multiple elements, use v-else-if to chain them instead.")
        );

        let ranged = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "<div></div><span></span><p></p>",
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(
            ranged["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
        assert_eq!(ranged["errors"].as_array().unwrap().len(), 1);
        assert_eq!(ranged["errors"][0]["start"], json!(11));
        assert!(ranged["errors"][0].get("end").is_none());
    }

    #[test]
    fn vue27_bridge_compile_template_projects_vue2_tip_ranges_from_compiler_options() {
        let plain = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><el-dropdown-item v-for="item in handle">{{ item.label }}</el-dropdown-item></div>"#,
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": false
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(plain["tips"].as_array().unwrap().len(), 1);
        assert_eq!(
            plain["tips"][0],
            json!(
                r#"<el-dropdown-item v-for="item in handle">: component lists rendered with v-for should have explicit keys. See https://v2.vuejs.org/v2/guide/list.html#key for more info."#
            )
        );

        let ranged = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><el-dropdown-item v-for="item in handle">{{ item.label }}</el-dropdown-item></div>"#,
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(ranged["tips"].as_array().unwrap().len(), 1);
        assert_eq!(
            ranged["tips"][0]["msg"],
            json!(
                r#"<el-dropdown-item v-for="item in handle">: component lists rendered with v-for should have explicit keys. See https://v2.vuejs.org/v2/guide/list.html#key for more info."#
            )
        );
        assert_eq!(ranged["tips"][0]["start"], json!(23));
        assert_eq!(ranged["tips"][0]["end"], json!(45));
        assert!(ranged["tips"][0].get("tip").is_none());

        let leading = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "\n<div><el-dropdown-item v-for=\"item in handle\">{{ item.label }}</el-dropdown-item></div>\n",
                "filename": "example.vue",
                "options": {
                    "compilerOptions": {
                        "outputSourceRange": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(leading["tips"].as_array().unwrap().len(), 1);
        assert_eq!(leading["tips"][0]["start"], json!(24));
        assert_eq!(leading["tips"][0]["end"], json!(46));
        assert!(leading["tips"][0].get("tip").is_none());
    }

    #[test]
    fn vue3_dom_bridge_projects_asset_url_imports() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png" srcset="./bar.png 2x">"#,
                "options": {
                    "mode": "module"
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!code.contains("_ctx._imports_"));

        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "mode": "module"
                }
            }),
        )
        .expect("dom parse");

        assert_eq!(parsed["imports"], json!([]));
    }

    #[test]
    fn vue3_sfc_compile_template_uses_bridge_options_for_hoist_static() {
        let compiled = dispatch(
            "sfc.compileTemplate",
            json!({
                "source": r#"<div><img src="./bar.png"><span>ok</span></div>"#,
                "filename": "template.vue",
                "options": {
                    "compilerOptions": {
                        "hoistStatic": false
                    }
                },
                "bridgeOptions": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "cacheHandlers": true,
                    "sourceMap": true,
                    "hoistStatic": false
                }
            }),
        )
        .expect("sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(!code.contains("_cache[0]"));
    }

    #[test]
    fn vue3_dom_bridge_stringifies_static_children_from_sentinel_option() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                "options": {
                    "prefixIdentifiers": true,
                    "hoistStatic": true,
                    "__vuecStringifyStatic": true
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("createStaticVNode"));
        assert!(code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn vue3_ssr_bridge_projects_asset_url_imports() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<img src="./bar.png" srcset="./bar.png 2x">"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(code.contains("_attrs"));
        assert!(!code.contains("_ctx._imports_"));

        let disabled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "transformAssetUrls": false
                }
            }),
        )
        .expect("ssr compile");

        let disabled_code = disabled["code"].as_str().unwrap_or("");
        assert!(!disabled_code.contains("import _imports_0"));
        assert!(disabled_code.contains(r#"src: "./bar.png""#));
        assert!(disabled_code.contains("_attrs"));
    }

    #[test]
    fn vue3_ssr_bridge_uses_dom_parser_defaults_for_components() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<router-link><img src="./logo.png"></router-link>"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("resolveComponent as _resolveComponent"));
        assert!(code.contains("const _component_router_link = _resolveComponent(\"router-link\")"));
        assert!(code.contains("_push(_ssrRenderComponent(_component_router_link, _attrs, {"));
        assert!(code.contains("_createVNode(\"img\", { src: _imports_0 })"));
    }
}
