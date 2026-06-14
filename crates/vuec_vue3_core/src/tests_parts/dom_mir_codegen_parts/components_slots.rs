    #[test]
    fn generate_vue3_dom_mir_emits_component_tags_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><Child/><Transition/><component :is="view"/></div>"#.into(),
            file_id: FileId(33),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(generated
            .code
            .contains("resolveDynamicComponent as _resolveDynamicComponent"));
        assert!(generated.code.contains("Transition as _Transition"));
        assert!(generated
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(generated.code.contains("_createVNode(_component_Child"));
        assert!(generated.code.contains("_createVNode(_Transition"));
        assert!(generated
            .code
            .contains("_createVNode(_resolveDynamicComponent(_ctx.view)"));
        assert!(!generated.code.contains("_component_Transition"));
        assert!(!generated.code.contains("is: _ctx.view"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_stable_component_slots_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #header="{ item }"><span>{{ item.name }}</span></template><p>{{ msg }}</p></Comp>"#.into(),
            file_id: FileId(35),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated
            .code
            .contains("const _component_Comp = _resolveComponent(\"Comp\")"));
        assert!(generated.code.contains("header: _withCtx(({ item }) => ["));
        assert!(generated.code.contains("_toDisplayString(item.name)"));
        assert!(generated.code.contains("default: _withCtx(() => ["));
        assert!(generated.code.contains("_toDisplayString(_ctx.msg)"));
        assert!(!generated.code.contains("_ctx.item"));
        assert!(generated.code.contains("_: 1"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_transition_persisted_prop_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Transition v-bind="base"><div v-show="ok"/></Transition>"#.into(),
            file_id: FileId(71),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("Transition as _Transition"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("vShow as _vShow"));
        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("_createBlock(_Transition"));
        assert!(generated.code.contains("_mergeProps(_ctx.base"));
        assert!(generated.code.contains("_ctx.base"));
        assert!(generated.code.contains("{ persisted: \"\" }"));
        assert!(generated.code.contains("[_vShow, _ctx.ok]"));
        assert!(generated.code.contains("512 /* NEED_PATCH */"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_dynamic_component_slots_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #[name]="slotProps"><span>{{ slotProps.label }}</span></template><template #fallback v-if="ok">Fallback</template><template #item v-for="(item, index) in list"><span>{{ item }}</span></template></Comp>"#.into(),
            file_id: FileId(37),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("createSlots as _createSlots"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("renderList as _renderList"));
        assert!(generated.code.contains("_createSlots({"));
        assert!(generated.code.contains("_: 2"));
        assert!(generated.code.contains("1024 /* DYNAMIC_SLOTS */"));
        assert!(generated.code.contains("name: _ctx.name"));
        assert!(generated.code.contains("fn: _withCtx((slotProps) => ["));
        assert!(generated.code.contains("_toDisplayString(slotProps.label)"));
        assert!(generated.code.contains("_ctx.ok"));
        assert!(generated.code.contains(": undefined"));
        assert!(generated.code.contains("name: \"fallback\""));
        assert!(generated.code.contains("key: \"1\""));
        assert!(generated
            .code
            .contains("_renderList(_ctx.list, (item, index) => {"));
        assert!(generated.code.contains("name: \"item\""));
        assert!(generated.code.contains("_toDisplayString(item)"));
        assert!(!generated.code.contains("_ctx.slotProps"));
        assert!(!generated.code.contains("_ctx.item"));
        assert!(!generated.code.contains("_ctx.index"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_dynamic_slot_if_else_alternates_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #one v-if="ok">One</template><template #two v-else-if="maybe">Two</template><template #fallback v-else>Fallback</template></Comp>"#.into(),
            file_id: FileId(40),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("createSlots as _createSlots"));
        assert!(generated.code.contains("_ctx.ok"));
        assert!(generated.code.contains("_ctx.maybe"));
        assert!(generated.code.contains("name: \"one\""));
        assert!(generated.code.contains("name: \"two\""));
        assert!(generated.code.contains("name: \"fallback\""));
        assert!(generated.code.contains("key: \"0\""));
        assert!(generated.code.contains("key: \"1\""));
        assert!(generated.code.contains("key: \"2\""));
        let ok_offset = generated.code.find("_ctx.ok").expect("ok condition");
        let maybe_offset = generated.code.find("_ctx.maybe").expect("maybe condition");
        let one_offset = generated.code.find("name: \"one\"").expect("one slot");
        let two_offset = generated.code.find("name: \"two\"").expect("two slot");
        let fallback_offset = generated
            .code
            .find("name: \"fallback\"")
            .expect("fallback slot");
        assert!(ok_offset < one_offset);
        assert!(one_offset < maybe_offset);
        assert!(maybe_offset < two_offset);
        assert!(two_offset < fallback_offset);
        assert!(!generated.code.contains(": undefined"));
        assert_eq!(generated.code.matches("name: \"one\"").count(), 1);
        assert_eq!(generated.code.matches("name: \"two\"").count(), 1);
        assert_eq!(generated.code.matches("name: \"fallback\"").count(), 1);
    }

    #[test]
    fn generate_vue3_dom_mir_emits_forwarded_component_slot_flag() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp><template #default><slot /></template></Comp>"#.into(),
            file_id: FileId(38),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("renderSlot as _renderSlot"));
        assert!(generated
            .code
            .contains("_renderSlot(_ctx.$slots, \"default\")"));
        assert!(generated.code.contains("_: 3"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_slot_outlet_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(42),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("renderSlot as _renderSlot"));
        assert!(generated
            .code
            .contains("toDisplayString as _toDisplayString"));
        assert!(generated
            .code
            .contains("_renderSlot(_ctx.$slots, _ctx.active, {"));
        assert!(generated.code.contains("foo: \"bar\""));
        assert!(generated.code.contains("baz: _ctx.baz"));
        assert!(generated.code.contains("() => ["));
        assert!(generated
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(!generated.code.contains("name: _ctx.active"));
    }
