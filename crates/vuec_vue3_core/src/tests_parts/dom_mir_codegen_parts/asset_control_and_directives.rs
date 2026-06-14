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
        assert!(generated.code.contains("item.ok"));
        assert!(generated.code.contains("? (_openBlock()"));
        assert!(generated.code.contains("_toDisplayString(item.name)"));
        assert!(!generated.code.contains("_ctx.item"));
        assert!(generated
            .code
            .contains("_createCommentVNode(\"v-if\", true)"));
        assert!(!generated.code.contains("v-for"));
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
