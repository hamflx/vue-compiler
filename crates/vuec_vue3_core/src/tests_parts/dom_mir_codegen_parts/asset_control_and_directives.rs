    #[test]
    fn generate_vue3_dom_mir_emits_basic_vnode_tree_without_ast() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :class="foo">{{ msg }}</div>"#.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert_eq!(generated.ast_summary, "vue3-dom-mir-nodes=3");
        assert!(generated
            .code
            .starts_with("const { toDisplayString: _toDisplayString"));
        assert!(generated
            .code
            .contains("return function render(_ctx, _cache)"));
        assert!(generated.code.contains("_createElementBlock(\"div\""));
        assert!(generated.code.contains("class: _normalizeClass(_ctx.foo)"));
        assert!(generated
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(generated.code.contains("3"));
        assert!(generated.code.contains("[\"class\"]"));
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_projects_asset_import_root_payload() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0">"#.into(),
                file_id: FileId(26),
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

        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let root_imports = match &result.mir.node(result.mir.root).unwrap().kind {
            Vue3DomMirKind::Root(root) => &root.imports,
            _ => unreachable!("DOM MIR root kind"),
        };
        assert_eq!(root_imports.len(), 1);
        assert_eq!(root_imports[0].name, "_imports_0");
        assert_eq!(root_imports[0].path, "./logo.png");
        let img = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3DomMirKind::VNodeCall(call) => Some(call),
                _ => None,
            })
            .unwrap();
        assert_eq!(img.dynamic_props, Vec::<String>::new());
        assert_eq!(img.patch_flag.bits, 0);
    }

    #[test]
    fn lower_vue3_ast_to_dom_mir_hoists_static_asset_import_bindings() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div><img :src="_imports_0" /><span :id="dynamic"></span></div>"#.into(),
                file_id: FileId(26),
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

        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        let hoists = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Hoisted { index } => Some((node.id, index)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hoists.len(), 1);
        assert_eq!(hoists[0].1, 1);
        assert_eq!(
            result.mir.node(hoists[0].0).map(|node| node.children.len()),
            Some(1)
        );

        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                mode: "module".into(),
                prefix_identifiers: true,
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(generated
            .code
            .contains("const _hoisted_1 = _createElementVNode(\"img\", { src: _imports_0 })"));
        assert!(generated.code.contains("_hoisted_1"));
        assert!(generated.code.contains("id: _ctx.dynamic"));
        assert!(!generated.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_asset_imports_from_mir_root() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0" :srcset="_imports_0 + ' 2x'">"#.into(),
                file_id: FileId(26),
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
            .contains("import _imports_0 from './logo.png'"));
        assert!(
            generated.code.find("from \"vue\"").unwrap()
                < generated.code.find("./logo.png").unwrap()
        );
        assert!(generated.code.contains("src: _imports_0"));
        assert!(generated.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!generated.code.contains("_ctx._imports_0"));
        assert!(!generated.code.contains("[\"src\""));
    }

    #[test]
    fn base_compile_hoists_static_srcset_asset_import_binding() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<img :srcset="_imports_0 + ' 2x'">"#.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            hoist_static: true,
            ..Vue3CompilerOptions::default()
        };
        let mut ast = Vue3Dialect::base_parse(source.clone(), &options);
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./logo.png".into(),
                });
            }
        }
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);
        let result = Vue3Dialect::finish_compile(ast, source, options, ctx);

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result
            .code
            .contains("const _hoisted_1 = _imports_0 + ' 2x'"));
        assert!(result.code.contains("srcset: _hoisted_1"));
        assert!(!result.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn base_compile_reuses_static_asset_fragment_binding_hoist() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r##"<svg><use :href="_imports_0 + '#fragment'"></use><use :href="_imports_0 + '#fragment'"></use></svg>"##.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            hoist_static: true,
            ..Vue3CompilerOptions::default()
        };
        let mut ast = Vue3Dialect::base_parse(source.clone(), &options);
        if let Some(root_node) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root_node.kind {
                root.imports.push(vuec_ast::Vue3ImportItem {
                    name: "_imports_0".into(),
                    path: "./icons.svg".into(),
                });
            }
        }
        let mut ctx = TransformContext::default();
        Vue3Dialect::transform(&mut ast, &mut ctx, &options);
        let result = Vue3Dialect::finish_compile(ast, source, options, ctx);

        assert!(result.code.contains("import _imports_0 from './icons.svg'"));
        assert!(result
            .code
            .contains("const _hoisted_1 = _imports_0 + '#fragment'"));
        assert_eq!(result.code.matches("const _hoisted_").count(), 1);
        assert_eq!(result.code.matches("href: _hoisted_1").count(), 2);
        assert!(!result.code.contains("_ctx._imports_0"));
    }

    #[test]
    fn generate_vue3_dom_mir_uses_function_mode_preamble_without_prefix() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div :class="foo">{{ msg }}</div>"#.into(),
            file_id: FileId(26),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());

        let generated =
            generate_vue3_dom_mir(&result.mir, &result.js, &Vue3CompilerOptions::default());

        assert!(generated.code.starts_with("const _Vue = Vue"));
        assert!(generated
            .code
            .contains("return function render(_ctx, _cache)"));
        assert!(generated.code.contains("with (_ctx) {"));
        assert!(generated
            .code
            .contains("const { toDisplayString: _toDisplayString"));
        assert!(generated.code.contains("class: _normalizeClass(foo)"));
        assert!(generated
            .code
            .contains("_createTextVNode(_toDisplayString(msg), 1 /* TEXT */)"));
        assert!(!generated.code.contains("export function render"));
        assert!(!generated.code.contains("_ctx.foo"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_control_flow_from_js_store() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="item in list" v-if="item.ok">{{ item.name }}</li>"#.into(),
            file_id: FileId(27),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(&ast, &Vue3CompilerOptions::default());
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated
            .code
            .contains("_renderList(_ctx.list, (item) => {"));
        let condition = generated
            .code
            .find("_ctx.item.ok")
            .expect("v-if condition");
        let loop_start = generated
            .code
            .find("_renderList(_ctx.list")
            .expect("v-for loop");
        assert!(condition < loop_start);
        assert!(generated
            .code
            .contains("_Fragment, { key: 0 }, _renderList"));
        assert!(generated.code.contains("_toDisplayString(item.name)"));
        assert!(!generated
            .code
            .contains("_toDisplayString(_ctx.item.name)"));
        assert!(generated
            .code
            .contains("_createCommentVNode(\"v-if\", true)"));
        assert!(!generated.code.contains("v-for"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_nested_if_else_alternates() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<p v-if="one">one</p><p v-else-if="two">two</p><p v-else>three</p>"#
                .into(),
            file_id: FileId(88),
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

        let first_condition = generated.code.find("_ctx.one").expect("first condition");
        let first_branch = generated
            .code
            .find("\"one\"")
            .expect("first branch");
        let second_condition = generated.code.find("_ctx.two").expect("second condition");
        let second_branch = generated
            .code
            .find("\"two\"")
            .expect("second branch");
        let alternate = generated
            .code
            .find("\"three\"")
            .expect("else branch");
        assert!(first_condition < first_branch);
        assert!(first_branch < second_condition);
        assert!(second_condition < second_branch);
        assert!(second_branch < alternate);
        assert!(!generated.code.contains("_createCommentVNode(\"v-if\""));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_structural_template_if_fragments() {
        let source = TemplateSource {
            filename: "structural-if.vue".into(),
            source: r#"<div v-if="first"/><template v-else-if="second"><span/></template><template v-else><i/><b/></template>"#
                .into(),
            file_id: FileId(123),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(
            generated
                .code
                .contains(r#"_createElementBlock("div", { key: 0 })"#),
            "{}",
            generated.code
        );
        assert!(
            generated
                .code
                .contains(r#"_createElementBlock("span", { key: 1 })"#),
            "{}",
            generated.code
        );
        assert!(
            generated
                .code
                .contains("_createElementBlock(_Fragment, { key: 2 }, ["),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(!generated.code.contains(r#"ElementBlock("template""#));
        assert!(!generated.code.contains(r#"ElementVNode("template""#));
    }

    #[test]
    fn generate_vue3_dom_mir_wraps_single_text_template_if_in_fragment_array() {
        let source = TemplateSource {
            filename: "structural-if-text.vue".into(),
            source: r#"<template v-if="ok">only</template>"#.into(),
            file_id: FileId(128),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(
            generated
                .code
                .contains("_createElementBlock(_Fragment, { key: 0 }, ["),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("_createTextVNode(\"only\")"));
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(!generated.code.contains(r#"ElementBlock("template""#));
    }

    #[test]
    fn generate_vue3_dom_mir_injects_key_into_structural_template_slot() {
        let source = TemplateSource {
            filename: "structural-if-slot.vue".into(),
            source: r#"<template v-if="ok"><slot/></template>"#.into(),
            file_id: FileId(129),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(
            generated
                .code
                .contains(r#"_renderSlot(_ctx.$slots, "default", { key: 0 })"#),
            "{}",
            generated.code
        );
        assert!(!generated.code.contains("_Fragment"));
        assert!(!generated.code.contains(r#"ElementBlock("template""#));
    }

    #[test]
    fn generate_vue3_dom_mir_keeps_structural_if_root_inside_render_with_hoists() {
        let source = TemplateSource {
            filename: "structural-if-hoist.vue".into(),
            source: r#"<template v-if="ok"><div/></template>"#.into(),
            file_id: FileId(130),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            hoist_static: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(
            generated
                .code
                .contains(r#"_createElementBlock("div", { key: 0 })"#),
            "{}",
            generated.code
        );
        assert!(!result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::Hoisted { .. })));
        assert!(!generated.code.contains("const _hoisted_1"));
    }

    #[test]
    fn generate_vue3_dom_mir_unwraps_keyed_single_child_template_v_for() {
        let source = TemplateSource {
            filename: "structural-for-single.vue".into(),
            source: r#"<template v-for="item in items" :key="item.id"><span/></template>"#
                .into(),
            file_id: FileId(124),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated
            .code
            .contains("_renderList(_ctx.items, (item) => {"));
        assert!(
            generated
                .code
                .contains(r#"return (_openBlock(), _createElementBlock("span", { key: item.id }))"#),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("128 /* KEYED_FRAGMENT */"));
        assert!(!generated.code.contains(r#"ElementBlock("template""#));
        assert!(!generated.code.contains(r#"ElementVNode("template""#));
    }

    #[test]
    fn generate_vue3_dom_mir_uses_vnode_for_stable_template_v_for_child() {
        let source = TemplateSource {
            filename: "stable-structural-for.vue".into(),
            source: r#"<template v-for="item in 10"><span/></template>"#.into(),
            file_id: FileId(127),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains(
            "(_openBlock(), _createElementBlock(_Fragment, null, _renderList(10, (item) => {"
        ));
        assert!(
            generated
                .code
                .contains(r#"return _createElementVNode("span")"#),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(!generated
            .code
            .contains(r#"_createElementBlock("span""#));
        assert!(!generated.code.contains(r#"ElementVNode("template""#));
    }

    #[test]
    fn generate_vue3_dom_mir_detects_literal_template_v_for_in_function_mode() {
        let source = TemplateSource {
            filename: "stable-structural-for-function.vue".into(),
            source: r#"<template v-for="item in 10"><span/></template>"#.into(),
            file_id: FileId(131),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains(
            "(_openBlock(), _createElementBlock(_Fragment, null, _renderList(10, (item) => {"
        ));
        assert!(
            generated
                .code
                .contains(r#"return _createElementVNode("span")"#),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(!generated.code.contains("_openBlock(true)"));
    }

    #[test]
    fn generate_vue3_dom_mir_keeps_stable_memoized_template_v_for_as_vnodes() {
        let source = TemplateSource {
            filename: "stable-memo-structural-for.vue".into(),
            source: r#"<template v-for="item in 10" :key="item" v-memo="[item]"><span/></template>"#
                .into(),
            file_id: FileId(132),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains(
            "(_openBlock(), _createElementBlock(_Fragment, null, _renderList(10, (item, __, ___, _cached) => {"
        ));
        assert!(generated.code.contains(
            r#"const _item = _createElementVNode("span", { key: item })"#
        ));
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(!generated.code.contains("_openBlock(true)"));
    }

    #[test]
    fn generate_vue3_dom_mir_keeps_stable_memoized_plain_v_for_as_blocks() {
        let source = TemplateSource {
            filename: "stable-memo-plain-for.vue".into(),
            source: r#"<div v-for="item in 10" v-memo="[item]"/>"#.into(),
            file_id: FileId(136),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains(
            "(_openBlock(), _createElementBlock(_Fragment, null, _renderList(10, (item, __, ___, _cached) => {"
        ));
        assert!(generated
            .code
            .contains(r#"const _item = (_openBlock(), _createElementBlock("div"))"#));
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(!generated
            .code
            .contains(r#"const _item = _createElementVNode("div")"#));
    }

    #[test]
    fn generate_vue3_dom_mir_caches_combined_structural_control_flow_once() {
        let source = TemplateSource {
            filename: "structural-once.vue".into(),
            source: r#"<template v-if="ok" v-for="item in items" v-once><span>{{ item }}</span></template><template v-else><i/></template>"#
                .into(),
            file_id: FileId(137),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert_eq!(
            generated
                .code
                .matches("_cache[0] || (_cache[0] =")
                .count(),
            1
        );
        assert!(!generated.code.contains("_cache[1]"));
        let cache = generated
            .code
            .find("_cache[0] || (_cache[0] =")
            .expect("control-flow cache");
        let condition = generated.code.find("_ctx.ok").expect("if condition");
        let loop_body = generated
            .code
            .find("_renderList(_ctx.items, (item) =>")
            .expect("v-for body");
        let alternate = generated
            .code
            .find(r#"_createElementBlock("i", { key: 1 })"#)
            .expect("else branch");
        assert!(cache < condition);
        assert!(condition < loop_body);
        assert!(loop_body < alternate);
        assert!(generated
            .code
            .contains("_Fragment, { key: 0 }, _renderList"));
    }

    #[test]
    fn generate_vue3_dom_mir_increments_keys_across_sibling_if_chains() {
        let source = TemplateSource {
            filename: "branch-keys.vue".into(),
            source: r#"<div v-if="a"/><template v-else><span/></template><template v-if="b"><i/></template><p v-else/>"#
                .into(),
            file_id: FileId(138),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        for (tag, key) in [("div", 0), ("span", 1), ("i", 2), ("p", 3)] {
            assert!(
                generated.code.contains(&format!(
                    r#"_createElementBlock("{tag}", {{ key: {key} }})"#
                )),
                "{}",
                generated.code
            );
        }
    }

    #[test]
    fn generate_vue3_dom_mir_keeps_dynamic_v_for_key_arguments_unkeyed() {
        let source = TemplateSource {
            filename: "dynamic-for-key.vue".into(),
            source: r#"<template v-for="item in items" :[key]="item.id"><span/></template><div v-for="entry in entries" :[key]="entry.id"/>"#
                .into(),
            file_id: FileId(139),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert_eq!(generated.code.matches("256 /* UNKEYED_FRAGMENT */").count(), 2);
        assert!(!generated.code.contains("128 /* KEYED_FRAGMENT */"));
        assert!(!generated.code.contains("key: item.id"));
        assert!(!generated.code.contains("key: entry.id"));
        assert!(!generated.code.contains("item.id"));
        assert!(generated.code.contains("entry.id"));
        assert!(generated.code.contains("[_ctx.key || \"\"]"));
    }

    #[test]
    fn generate_vue3_dom_mir_keys_valueless_v_for_fragments() {
        let source = TemplateSource {
            filename: "valueless-for-key.vue".into(),
            source: r#"<template v-for="item in items" key><span/></template><div v-for="entry in entries" key/>"#
                .into(),
            file_id: FileId(141),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert_eq!(generated.code.matches("128 /* KEYED_FRAGMENT */").count(), 2);
        assert!(!generated.code.contains("256 /* UNKEYED_FRAGMENT */"));
        assert!(generated.code.contains(r#"_createElementBlock("span")"#));
        assert!(generated
            .code
            .contains(r#"_createElementBlock("div", { key: "" })"#));
        assert!(!generated.code.contains(r#"_createElementBlock("span", { key:"#));
    }

    #[test]
    fn generate_vue3_dom_mir_preserves_legacy_keyed_for_metadata() {
        let source = TemplateSource {
            filename: "legacy-keyed-for.vue".into(),
            source: r#"<div v-for="item in items" :key="item.id"/>"#.into(),
            file_id: FileId(142),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let mut result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let for_mir = result
            .mir
            .nodes
            .iter_mut()
            .find_map(|node| match &mut node.kind {
                Vue3DomMirKind::For(for_mir) => Some(for_mir),
                _ => None,
            })
            .expect("keyed loop");
        assert!(for_mir.key.is_some());
        for_mir.has_key = false;

        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);
        assert!(generated.code.contains("128 /* KEYED_FRAGMENT */"));
        assert!(!generated.code.contains("256 /* UNKEYED_FRAGMENT */"));
    }

    #[test]
    fn generate_vue3_dom_mir_ignores_once_on_non_initial_if_branches() {
        let source = TemplateSource {
            filename: "non-initial-branch-once.vue".into(),
            source: r#"<div v-if="a"/><span v-else v-once><i v-once/></span><template v-if="b"><b/></template><template v-else v-once><em v-once/></template>"#
                .into(),
            file_id: FileId(143),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(!result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3DomMirKind::Cache { .. })));
        assert!(!generated.code.contains("_cache["));
        assert!(generated
            .code
            .contains(r#"_createElementBlock("span", { key: 1 }"#));
        assert!(generated
            .code
            .contains(r#"_createElementBlock("em", { key: 3 })"#));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_template_v_for_fragments_and_if_precedence() {
        let source = TemplateSource {
            filename: "structural-for-fragment.vue".into(),
            source: r#"<template v-if="show" v-for="item in items">hello<i>{{ item }}</i></template>"#
                .into(),
            file_id: FileId(125),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_dom_mir(&ast, &options);
        let generated = generate_vue3_dom_mir(&result.mir, &result.js, &options);

        assert!(generated.code.contains("_ctx.show"));
        assert!(generated.code.contains(
            "_createElementBlock(_Fragment, { key: 0 }, _renderList(_ctx.items, (item) => {"
        ));
        assert!(
            generated
                .code
                .contains("return (_openBlock(), _createElementBlock(_Fragment, null, ["),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("64 /* STABLE_FRAGMENT */"));
        assert!(generated.code.contains("256 /* UNKEYED_FRAGMENT */"));
        assert!(generated.code.contains("_toDisplayString(item)"));
        assert!(!generated.code.contains("_toDisplayString(_ctx.item)"));
        assert!(!generated.code.contains(r#"ElementBlock("template""#));
        assert!(!generated.code.contains(r#"ElementVNode("template""#));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_cache_and_hoist_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span id="one">one</span><p v-once>{{ msg }}</p></div>"#.into(),
            file_id: FileId(28),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_dom_mir(
            &ast,
            &Vue3CompilerOptions {
                hoist_static: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let generated = generate_vue3_dom_mir(
            &result.mir,
            &result.js,
            &Vue3CompilerOptions {
                hoist_static: true,
                prefix_identifiers: true,
                ..Vue3CompilerOptions::default()
            },
        );

        assert!(generated.code.contains("_hoisted_1"));
        assert!(generated
            .code
            .contains("const _hoisted_1 = _createElementVNode(\"span\""));
        assert!(generated
            .code
            .contains("return function render(_ctx, _cache)"));
        assert!(generated.code.contains("_cache[0] || (_cache[0] ="));
        assert!(generated.code.contains("_toDisplayString(_ctx.msg)"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_memo_wrappers() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><p v-memo="[msg]">{{ msg }}</p></div>"#.into(),
            file_id: FileId(30),
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

        assert!(generated.code.contains("withMemo as _withMemo"));
        assert!(generated
            .code
            .contains("_withMemo([_ctx.msg], () => (_openBlock(), _createElementBlock(\"p\""));
        assert!(generated.code.contains("_cache, 0)"));
        assert!(generated.code.contains("_toDisplayString(_ctx.msg)"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_for_memo_cache_target() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="{ x, y } in list" :key="x" v-memo="[x, y === z]"><span>foobar</span></div>"#.into(),
            file_id: FileId(31),
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

        assert!(generated.code.contains("isMemoSame as _isMemoSame"));
        assert!(generated
            .code
            .contains("_renderList(_ctx.list, ({ x, y }, __, ___, _cached) =>"));
        assert!(generated.code.contains("const _memo = ([x, y === _ctx.z])"));
        assert!(generated
            .code
            .contains("_cached.key === x && _isMemoSame(_cached, _memo)"));
        assert!(generated
            .code
            .contains("const _item = (_openBlock(), _createElementBlock(\"div\""));
        assert!(generated.code.contains("_item.memo = _memo"));
        assert!(generated
            .code
            .contains("}, _cache, 0), 128 /* KEYED_FRAGMENT */)"));
        assert!(!generated.code.contains("_ctx.x"));
        assert!(!generated.code.contains("_ctx.y"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_content_override_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><section v-html="raw">old</section><p v-text="msg"/><span>keep</span></div>"#.into(),
            file_id: FileId(59),
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
            .contains("toDisplayString as _toDisplayString"));
        assert!(generated.code.contains("{ innerHTML: _ctx.raw }"));
        assert!(generated
            .code
            .contains("textContent: _toDisplayString(_ctx.msg)"));
        assert!(generated.code.contains("8 /* PROPS */, [\"innerHTML\"]"));
        assert!(generated.code.contains("8 /* PROPS */, [\"textContent\"]"));
        assert!(generated.code.contains("\"keep\""));
        assert!(!generated.code.contains("\"old\""));
    }

    #[test]
    fn generate_vue3_dom_mir_merges_content_override_with_object_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-bind="before" v-html="raw"/><p v-text="'hi'" v-bind="after"/>"#
                .into(),
            file_id: FileId(60),
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

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated
            .code
            .contains("_mergeProps(_ctx.before, { innerHTML: _ctx.raw })"));
        assert!(generated
            .code
            .contains("_mergeProps({ textContent: 'hi' }, _ctx.after)"));
        assert!(!generated.code.contains("_toDisplayString('hi')"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_native_v_model_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><input v-model="text"><input type="radio" v-model="picked"><input type="checkbox" v-model.trim="checked"><input :type="kind" v-model="dynamic"><select v-model="selected"/><textarea v-model.lazy="body"/></div>"#.into(),
            file_id: FileId(62),
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

        assert!(generated.code.contains("vModelText as _vModelText"));
        assert!(generated.code.contains("vModelRadio as _vModelRadio"));
        assert!(generated.code.contains("vModelCheckbox as _vModelCheckbox"));
        assert!(generated.code.contains("vModelSelect as _vModelSelect"));
        assert!(generated.code.contains("vModelDynamic as _vModelDynamic"));
        assert!(generated.code.contains("withDirectives as _withDirectives"));
        assert!(generated.code.contains("[_vModelText, _ctx.text]"));
        assert!(generated.code.contains("[_vModelRadio, _ctx.picked]"));
        assert!(generated.code.contains("_vModelCheckbox"));
        assert!(generated.code.contains("_ctx.checked"));
        assert!(generated.code.contains("trim: true"));
        assert!(generated.code.contains("[_vModelDynamic, _ctx.dynamic]"));
        assert!(generated.code.contains("[_vModelSelect, _ctx.selected]"));
        assert!(generated.code.contains("_ctx.body"));
        assert!(generated.code.contains("lazy: true"));
        assert!(generated
            .code
            .contains("\"onUpdate:modelValue\": $event => ((_ctx.text) = $event)"));
        assert!(generated
            .code
            .contains("\"onUpdate:modelValue\": $event => ((_ctx.body) = $event)"));
        assert!(generated
            .code
            .contains("8 /* PROPS */, [\"onUpdate:modelValue\"]"));
    }

    #[test]
    fn generate_vue3_dom_mir_merges_native_v_model_with_object_bindings() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<input v-bind="before" v-model="text" class="field"><input v-model="after" v-bind="tail">"#.into(),
            file_id: FileId(63),
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

        assert!(generated.code.contains("mergeProps as _mergeProps"));
        assert!(generated.code.contains("_mergeProps(_ctx.before"));
        assert!(generated.code.contains("class: \"field\""));
        assert!(generated.code.contains("_mergeProps({ \"onUpdate:modelValue\": $event => ((_ctx.after) = $event) }, _ctx.tail)"));
        assert!(generated.code.contains("[_vModelDynamic, _ctx.text]"));
        assert!(generated.code.contains("[_vModelDynamic, _ctx.after]"));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_runtime_directives_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-focus:foo.bar="value"><span v-show="ok"/></div>"#.into(),
            file_id: FileId(29),
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

        assert!(generated.code.contains("withDirectives as _withDirectives"));
        assert!(generated
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(generated.code.contains("vShow as _vShow"));
        assert!(generated
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(generated
            .code
            .contains("[_directive_focus, _ctx.value, \"foo\", {"));
        assert!(generated.code.contains("bar: true"));
        assert!(generated.code.contains("[_vShow, _ctx.ok]"));
        assert!(!generated.code.contains("_resolveDirective(\"show\")"));
        assert!(!result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3DomMirKind::VNodeCall(call)
                if call.directives.iter().any(|directive| directive.name == "show")
        )));
    }

    #[test]
    fn generate_vue3_dom_mir_emits_v_show_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><span v-show="ok"/><p v-focus v-show="visible"/></div>"#.into(),
            file_id: FileId(69),
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

        assert!(generated.code.contains("withDirectives as _withDirectives"));
        assert!(generated.code.contains("vShow as _vShow"));
        assert!(generated
            .code
            .contains("resolveDirective as _resolveDirective"));
        assert!(generated
            .code
            .contains("const _directive_focus = _resolveDirective(\"focus\")"));
        assert!(generated.code.contains("[_vShow, _ctx.ok]"));
        assert!(generated.code.contains("[_vShow, _ctx.visible]"));
        assert!(generated.code.contains("[_directive_focus]"));
        assert!(generated.code.contains("512 /* NEED_PATCH */"));
        assert!(!generated.code.contains("_resolveDirective(\"show\")"));
    }
