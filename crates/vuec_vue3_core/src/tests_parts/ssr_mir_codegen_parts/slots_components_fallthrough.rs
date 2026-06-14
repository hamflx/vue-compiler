    #[test]
    fn generate_vue3_ssr_mir_emits_slot_outlet_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(44),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderSlot as _ssrRenderSlot"));
        assert!(generated.code.contains("ssrInterpolate as _ssrInterpolate"));
        assert!(generated
            .code
            .contains("_ssrRenderSlot(_ctx.$slots, _ctx.active, {"));
        assert!(generated.code.contains("foo: \"bar\""));
        assert!(generated.code.contains("baz: _ctx.baz"));
        assert!(generated.code.contains("() => {"));
        assert!(generated
            .code
            .contains("_push(`fallback ${_ssrInterpolate(_ctx.msg)}`)"));
        assert!(!generated.code.contains("name: _ctx.active"));
    }

    #[test]
    fn generate_vue3_ssr_mir_keeps_slot_object_props_as_props() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot name="default" v-bind="slotProps" v-on="listeners" :foo="value" />"#
                .into(),
            file_id: FileId(47),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("toHandlers as _toHandlers"));
        assert!(generated
            .code
            .contains("_ssrRenderSlot(_ctx.$slots, \"default\", _normalizeProps("));
        assert!(generated.code.contains("_mergeProps("));
        assert!(generated.code.contains("_ctx.slotProps"));
        assert!(generated.code.contains("_toHandlers(_ctx.listeners, true)"));
        assert!(generated.code.contains("foo: _ctx.value"));
        assert!(!generated.code.contains("_ssrRenderAttrs(_ctx.slotProps)"));
        assert!(!generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_dynamic_component_vnode_path() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<component :is="view" prop="b" v-bind="attrs"><span>hi</span></component><component is="plain" />"#.into(),
            file_id: FileId(78),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let components = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => Some(component),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(components.len(), 2);
        assert!(components.iter().all(|component| component.dynamic));
        assert!(components.iter().all(|component| component
            .props
            .static_attrs
            .iter()
            .all(|attr| attr.name != "is")));
        assert!(components.iter().all(|component| component
            .props
            .dynamic_bindings
            .iter()
            .all(|binding| binding.name != "is")));

        let generated = generate_vue3_ssr_mir(
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
            .contains("resolveDynamicComponent as _resolveDynamicComponent"));
        assert!(generated.code.contains("createVNode as _createVNode"));
        assert!(generated.code.contains("ssrRenderVNode as _ssrRenderVNode"));
        assert!(!generated
            .code
            .contains("ssrRenderComponent as _ssrRenderComponent"));
        assert!(generated
            .code
            .contains("_ssrRenderVNode(_push, _createVNode(_resolveDynamicComponent(_ctx.view),"));
        assert!(generated
            .code
            .contains("_createVNode(_resolveDynamicComponent(\"plain\"), null, null)"));
        assert!(generated.code.contains("prop: \"b\""));
        assert!(generated.code.contains("_ctx.attrs"));
        assert!(!generated.code.contains("is: _ctx.view"));
        assert!(!generated.code.contains("is: \"plain\""));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_control_flow_and_components_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Comp foo="bar" :baz="baz" v-bind="extra"><p v-if="ok">yes</p><p v-else-if="maybe">maybe</p><p v-else>no</p><li v-for="item in list">{{ item.name }}</li></Comp>"#.into(),
            file_id: FileId(45),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => Some(component),
                _ => None,
            })
            .expect("ssr component");
        assert_eq!(component.props.static_attrs.len(), 1);
        assert_eq!(component.props.static_attrs[0].name, "foo");
        assert_eq!(component.props.dynamic_bindings.len(), 1);
        assert_eq!(component.props.dynamic_bindings[0].name, "baz");
        assert_eq!(component.props.object_bindings.len(), 1);
        let generated = generate_vue3_ssr_mir(
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
            .contains("ssrRenderComponent as _ssrRenderComponent"));
        assert!(generated
            .code
            .contains("resolveComponent as _resolveComponent"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("ssrRenderList as _ssrRenderList"));
        assert!(generated
            .code
            .contains("const _component_Comp = _resolveComponent(\"Comp\")"));
        assert!(generated
            .code
            .contains("_push(_ssrRenderComponent(_component_Comp, _mergeProps("));
        assert!(generated
            .code
            .contains("default: _withCtx((_, _push, _parent, _scopeId) => {"));
        assert!(generated.code.contains("_mergeProps({"));
        assert!(generated.code.contains("foo: \"bar\""));
        assert!(generated.code.contains("baz: _ctx.baz"));
        assert!(generated.code.contains("_ctx.extra"));
        assert!(generated.code.contains("if (_ctx.ok) {"));
        assert!(generated.code.contains("} else if (_ctx.maybe) {"));
        assert!(!generated.code.contains("} else {\n        if (_ctx.maybe)"));
        let ok_offset = generated.code.find("if (_ctx.ok)").expect("ok branch");
        let maybe_offset = generated
            .code
            .find("} else if (_ctx.maybe)")
            .expect("maybe branch");
        let yes_offset = generated
            .code
            .find("_push(`<p${_scopeId}>yes</p>`)")
            .expect("yes body");
        let maybe_body_offset = generated
            .code
            .find("_push(`<p${_scopeId}>maybe</p>`)")
            .expect("maybe body");
        let no_offset = generated
            .code
            .find("_push(`<p${_scopeId}>no</p>`)")
            .expect("else body");
        assert!(ok_offset < yes_offset);
        assert!(yes_offset < maybe_offset);
        assert!(maybe_offset < maybe_body_offset);
        assert!(maybe_body_offset < no_offset);
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.list, (item) => {"));
        assert!(generated.code.contains("_ssrInterpolate(item.name)"));
        assert!(!generated.code.contains("_ctx.item"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_template_v_if_fragments_and_root_attrs() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<template v-if="foo"><div>hi</div><div>ho</div></template><div v-else/>"#
                .into(),
            file_id: FileId(82),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("if (_ctx.foo) {"));
        assert!(generated
            .code
            .contains("_push(`<!--[--><div>hi</div><div>ho</div><!--]-->`)"));
        assert!(generated.code.contains("} else {"));
        assert!(generated
            .code
            .contains("_push(`<div${_ssrRenderAttrs(_attrs)}></div>`)"));
        assert!(!generated.code.contains("<template"));
    }

    #[test]
    fn generate_vue3_ssr_mir_injects_root_attrs_through_if_branches() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-if="foo"></div><span v-else-if="bar"></span><p v-else></p>"#.into(),
            file_id: FileId(83),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(generated.code.contains("if (_ctx.foo) {"));
        assert!(generated.code.contains("} else if (_ctx.bar) {"));
        assert!(generated
            .code
            .contains("_push(`<div${_ssrRenderAttrs(_attrs)}></div>`)"));
        assert!(generated
            .code
            .contains("_push(`<span${_ssrRenderAttrs(_attrs)}></span>`)"));
        assert!(generated
            .code
            .contains("_push(`<p${_ssrRenderAttrs(_attrs)}></p>`)"));
    }

    #[test]
    fn generate_vue3_ssr_mir_injects_fallthrough_attrs_through_root_comments() {
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            built_in_components: vec!["transition".into()],
            ..Vue3CompilerOptions::default()
        };
        let render = |source: &str, file_id: u32| {
            let source = TemplateSource {
                filename: "foo.vue".into(),
                source: source.into(),
                file_id: FileId(file_id),
                base_offset: 0,
            };
            let ast = Vue3Dialect::base_parse(source, &options);
            let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
            generate_vue3_ssr_mir(&result.mir, &result.js, &options).code
        };

        let commented_root = render(r#"<!--!--><div/>"#, 84);
        assert!(commented_root.contains("ssrRenderAttrs as _ssrRenderAttrs"));
        assert!(commented_root
            .contains(r#"_push(`<!--[--><!--!--><div${_ssrRenderAttrs(_attrs)}></div><!--]-->`)"#));

        let transition_root = render(r#"<!--root--><transition><div/></transition>"#, 85);
        assert!(transition_root.contains(
            r#"_push(`<!--[--><!--root--><div${_ssrRenderAttrs(_attrs)}></div><!--]-->`)"#
        ));

        let dynamic_fragment = render(r#"<div v-if="true"/><div/>"#, 86);
        assert!(!dynamic_fragment.contains("_ssrRenderAttrs(_attrs)"));
        assert!(dynamic_fragment.contains(r#"_push(`<div></div><!--]-->`)"#));
    }

    #[test]
    fn generate_vue3_ssr_mir_injects_css_vars_across_fragments_and_suspense() {
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ssr_css_vars: Some("{ color }".into()),
            ..Vue3CompilerOptions::default()
        };
        let render = |source: &str, file_id: u32| {
            let source = TemplateSource {
                filename: "foo.vue".into(),
                source: source.into(),
                file_id: FileId(file_id),
                base_offset: 0,
            };
            let ast = Vue3Dialect::base_parse(source, &options);
            let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
            generate_vue3_ssr_mir(&result.mir, &result.js, &options).code
        };

        let basic = render(r#"<div/>"#, 87);
        assert!(basic.contains("const _cssVars = { style: { color: _ctx.color }}"));
        assert!(basic.contains(r#"_ssrRenderAttrs(_mergeProps(_attrs, _cssVars))"#));

        let multiline_options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ssr_css_vars: Some("{\n  \":--x-color\": (color),\n  \":--x-size\": (size)\n}".into()),
            ..Vue3CompilerOptions::default()
        };
        let multiline_source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div/>"#.into(),
            file_id: FileId(91),
            base_offset: 0,
        };
        let multiline_ast = Vue3Dialect::base_parse(multiline_source, &multiline_options);
        let multiline = lower_vue3_ast_to_ssr_mir(&multiline_ast, &multiline_options);
        let multiline_code =
            generate_vue3_ssr_mir(&multiline.mir, &multiline.js, &multiline_options).code;
        assert!(multiline_code.contains(
            "const _cssVars = { style: {\n  \":--x-color\": (_ctx.color),\n  \":--x-size\": (_ctx.size)\n}}"
        ));

        let fragment = render(r#"<div/><div/>"#, 88);
        assert_eq!(fragment.matches("_ssrRenderAttrs(_cssVars)").count(), 2);
        assert!(!fragment.contains("_mergeProps(_attrs, _cssVars)"));

        let branches = render(
            r#"<div v-if="ok"/><template v-else><div/><div/></template>"#,
            89,
        );
        assert!(branches.contains(r#"_ssrRenderAttrs(_mergeProps(_attrs, _cssVars))"#));
        assert_eq!(branches.matches("_ssrRenderAttrs(_cssVars)").count(), 2);

        let suspense = render(
            r#"<Suspense><div>ok</div><template #fallback><div>fallback</div></template></Suspense>"#,
            90,
        );
        assert!(suspense.contains(r#"_push(`<div${_ssrRenderAttrs(_cssVars)}>fallback</div>`)"#));
        assert!(suspense.contains(r#"_push(`<div${_ssrRenderAttrs(_cssVars)}>ok</div>`)"#));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_component_slot_vnode_asset_fallback() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<Comp><img :src="_imports_0"></Comp>"#.into(),
                file_id: FileId(76),
                base_offset: 0,
            },
            &Vue3CompilerOptions::default(),
        );
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }

        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_ssr_mir(
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
            .contains("import { resolveComponent as _resolveComponent, withCtx as _withCtx, createVNode as _createVNode } from \"vue\""));
        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(generated
            .code
            .contains("const _component_Comp = _resolveComponent(\"Comp\")"));
        assert!(generated
            .code
            .contains("_ssrRenderAttr(\"src\", _imports_0)"));
        assert!(generated
            .code
            .contains("_createVNode(\"img\", { src: _imports_0 })"));
        assert!(!generated.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_scope_id_slot_vnode_fallback() {
        let text_source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo>foo</foo>"#.into(),
            file_id: FileId(77),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            scope_id: Some("data-v-xxxxxxx".into()),
            native_tags: Some(vec!["span".into(), "div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let text_ast = Vue3Dialect::base_parse(text_source, &options);
        let text_result = lower_vue3_ast_to_ssr_mir(&text_ast, &options);
        let text_generated = generate_vue3_ssr_mir(&text_result.mir, &text_result.js, &options);
        assert!(text_generated
            .code
            .contains("createTextVNode as _createTextVNode"));
        assert!(text_generated
            .code
            .contains("return [\n          _createTextVNode(\"foo\")\n        ]"));
        assert!(!text_generated
            .code
            .contains("return [\n          \"foo\"\n        ]"));

        let nested_source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo><span>hello</span><bar><span/></bar></foo>"#.into(),
            file_id: FileId(78),
            base_offset: 0,
        };
        let nested_ast = Vue3Dialect::base_parse(nested_source, &options);
        let nested_result = lower_vue3_ast_to_ssr_mir(&nested_ast, &options);
        let nested_generated =
            generate_vue3_ssr_mir(&nested_result.mir, &nested_result.js, &options);
        assert!(nested_generated
            .code
            .contains("_push(`<span data-v-xxxxxxx${_scopeId}>hello</span>`)"));
        assert!(nested_generated.code.contains("}, _parent, _scopeId)"));
        assert!(nested_generated
            .code
            .contains("_createVNode(\"span\", null, \"hello\")"));
        assert!(nested_generated.code.contains(
            "_createVNode(_component_bar, null, {\n            default: _withCtx(() => ["
        ));
        assert!(nested_generated.code.contains("_createVNode(\"span\")"));
        assert!(!nested_generated.code.contains("\"data-v-xxxxxxx\": \"\""));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_component_slots_and_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo v-xxx:x.y="z"><template v-slot:named v-if="ok">foo</template><template v-for="(key, index) in names" v-slot:[key]="{ msg }">{{ msg + key + index + bar }}</template></foo>"#.into(),
            file_id: FileId(92),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            native_tags: Some(vec!["div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let component = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderComponent(component) => Some(component),
                _ => None,
            })
            .expect("ssr component");

        assert_eq!(component.directives.len(), 1);
        assert_eq!(component.directives[0].name, "xxx");
        assert_eq!(component.directives[0].argument.as_deref(), Some("x"));
        assert_eq!(component.directives[0].modifiers, vec!["y"]);
        assert!(component.directives[0].expression.is_some());

        let slots = component.slots.as_ref().expect("component slots");
        assert_eq!(slots.flag, Vue3SlotFlag::Dynamic);
        assert!(slots.dynamic_slots.iter().any(|slot| matches!(
            slot,
            vuec_ast::Vue3DomDynamicSlot::Conditional(slot)
                if slot.condition.is_some()
                    && matches!(slot.slot.name, Vue3DomSlotName::Static(ref name) if name == "named")
                    && slot.slot.key.as_deref() == Some("0")
        )));
        assert!(slots.dynamic_slots.iter().any(|slot| matches!(
            slot,
            vuec_ast::Vue3DomDynamicSlot::For(slot)
                if matches!(slot.slot.name, Vue3DomSlotName::Dynamic(_))
                    && slot.key_alias.is_some()
                    && slot.index_alias.is_none()
                    && slot.slot.params.is_some()
        )));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_component_slots_and_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo v-xxx:x.y="z"><template v-slot:named v-if="ok">foo</template><template v-for="(key, index) in names" v-slot:[key]="{ msg }">{{ msg + key + index + bar }}</template></foo>"#.into(),
            file_id: FileId(93),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            prefix_identifiers: true,
            scope_id: Some("data-v-test".into()),
            native_tags: Some(vec!["div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
        let generated = generate_vue3_ssr_mir(&result.mir, &result.js, &options);

        assert!(generated
            .code
            .contains("resolveDirective: _resolveDirective"));
        assert!(generated
            .code
            .contains("ssrGetDirectiveProps: _ssrGetDirectiveProps"));
        assert!(generated
            .code
            .contains(r#"_ssrGetDirectiveProps(_ctx, _directive_xxx, _ctx.z, "x", { y: true })"#));
        assert!(generated
            .code
            .contains("_createSlots({ _: 2 /* DYNAMIC */ }, ["));
        assert!(generated
            .code
            .contains("(_ctx.ok)\n      ? {\n          name: \"named\",\n          fn: _withCtx"));
        assert!(generated
            .code
            .contains("_renderList(_ctx.names, (key, index) => {"));
        assert!(generated
            .code
            .contains("_push(`${_ssrInterpolate(msg + key + index + _ctx.bar)}`)"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_transition_vnode_fallback_in_component_slot() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<foo><transition><div v-if="false"/></transition></foo>"#.into(),
            file_id: FileId(94),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            scope_id: Some("data-v-test".into()),
            native_tags: Some(vec!["div".into()]),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
        let generated = generate_vue3_ssr_mir(&result.mir, &result.js, &options);

        assert!(generated
            .code
            .contains("_push(`<div data-v-test${_scopeId}></div>`)"));
        assert!(generated.code.contains("_createVNode(_Transition, null, {"));
        assert!(generated.code.contains("default: _withCtx(() => ["));
        assert!(generated
            .code
            .contains(r#"_openBlock(), _createBlock("div", { key: 0 })"#));
    }
