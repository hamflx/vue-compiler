    #[test]
    fn lower_vue3_ast_to_ssr_mir_records_target_split_edges_and_js_store() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div id=\"app\" :class=\"foo\">{{ msg }}</div>".into(),
            file_id: FileId(11),
            base_offset: 5,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert!(result
            .map
            .hir_for_ast(ast.root)
            .any(|hir| hir == result.hir.root));
        assert!(result
            .map
            .mir_for_hir(result.hir.root)
            .any(|mir| mir == result.mir.root));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["foo", "msg"]
        );

        let pushes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pushes, vec!["<div id=\"app\"", ">", "</div>"]);
        assert!(result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3SsrMirKind::RenderAttrs(attrs)
                if attrs.props.dynamic_bindings.len() == 1
                    && attrs.props.dynamic_bindings[0].name == "class"
                    && attrs.props.dynamic_bindings[0].value == JsExprId(0)
        )));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::PushInterpolated(MirExpr::JsExpr(_))
        )));
        assert!(result
            .mir
            .nodes
            .iter()
            .all(|node| !matches!(node.kind, Vue3SsrMirKind::RenderComponent(_))));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_asset_import_root_payload() {
        let mut ast = Vue3Dialect::base_parse(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<img :src="_imports_0">"#.into(),
                file_id: FileId(72),
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
        let root_imports = match &result.mir.node(result.mir.root).unwrap().kind {
            Vue3SsrMirKind::Root(root) => &root.imports,
            _ => unreachable!("SSR MIR root kind"),
        };
        assert_eq!(root_imports.len(), 1);
        assert_eq!(root_imports[0].name, "_imports_0");
        assert_eq!(root_imports[0].path, "./logo.png");
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_keeps_component_and_slot_target_split() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Comp><slot name=\"header\">fallback</slot></Comp>".into(),
            file_id: FileId(12),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::lower_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::Component(_))));
        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::SlotOutlet(_))));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3SsrMirKind::RenderComponent(component)
                if matches!(component.tag, MirExpr::String(_))
                    && component.props.static_attrs.is_empty()
        )));
        assert!(result.mir.nodes.iter().any(|node| matches!(
            &node.kind,
            Vue3SsrMirKind::RenderSlot(slot)
                if matches!(slot.name, Vue3DomSlotName::Static(ref name) if name == "header")
                    && slot.fallback.len() == 1
        )));
        assert!(result
            .mir
            .nodes
            .iter()
            .all(|node| !matches!(node.kind, Vue3SsrMirKind::PushInterpolated(_))));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_slot_outlet_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<slot :name="active" foo="bar" :baz="baz">fallback {{ msg }}</slot>"#.into(),
            file_id: FileId(42),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = Vue3Dialect::lower_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let slot = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::RenderSlot(slot) => Some(slot),
                _ => None,
            })
            .expect("ssr render slot");
        assert!(matches!(slot.name, Vue3DomSlotName::Dynamic(JsExprId(0))));
        assert_eq!(slot.props.static_attrs.len(), 1);
        assert_eq!(slot.props.static_attrs[0].name, "foo");
        assert_eq!(slot.props.static_attrs[0].value, "bar");
        assert_eq!(slot.props.dynamic_bindings.len(), 1);
        assert_eq!(slot.props.dynamic_bindings[0].name, "baz");
        assert_eq!(slot.props.dynamic_bindings[0].value, JsExprId(1));
        assert_eq!(slot.fallback.len(), 2);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["active", "baz", "msg"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_lowers_v_for_and_v_if_control_flow() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="item in list" v-if="item.ok">{{ item.name }}</li>"#.into(),
            file_id: FileId(14),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["list", "item.ok", "item.name"]
        );
        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["item"]
        );

        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::For(_))));
        assert!(result
            .hir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, HirNodeKind::If(_))));
        let for_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::For(for_mir) => Some(for_mir),
                _ => None,
            })
            .expect("SSR MIR for");
        assert_eq!(for_mir.source, JsExprId(0));
        assert_eq!(for_mir.value_alias, JsPatternId(0));
        assert!(for_mir.key_alias.is_none());
        assert!(for_mir.index_alias.is_none());
        assert!(result.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue3SsrMirKind::If {
                condition: Some(_),
                ..
            }
        )));
        assert!(result
            .mir
            .nodes
            .iter()
            .any(|node| matches!(node.kind, Vue3SsrMirKind::PushInterpolated(_))));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_v_for_alias_payload() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="(item, key, index) in list">{{ key }}:{{ index }}:{{ item.name }}</li>"#.into(),
            file_id: FileId(18),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["list", "key", "index", "item.name"]
        );
        assert_eq!(
            result
                .js
                .patterns()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["item", "key", "index"]
        );

        let for_mir = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::For(for_mir) => Some(for_mir),
                _ => None,
            })
            .expect("SSR MIR for");
        assert_eq!(for_mir.source, JsExprId(0));
        assert_eq!(for_mir.value_alias, JsPatternId(0));
        assert_eq!(for_mir.key_alias, Some(JsPatternId(1)));
        assert_eq!(for_mir.index_alias, Some(JsPatternId(2)));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_projects_builtin_component_payloads() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r##"<Teleport :to="target" :disabled="off"><div id="x"/></Teleport><Suspense><template #default><Foo/></template><template #fallback>loading</template></Suspense>"##.into(),
            file_id: FileId(19),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let teleport = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::Teleport(teleport) => Some(teleport),
                _ => None,
            })
            .expect("SSR MIR teleport");
        assert_eq!(teleport.target, MirExpr::JsExpr(JsExprId(0)));
        assert_eq!(teleport.disabled, MirExpr::JsExpr(JsExprId(1)));

        let suspense = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::Suspense(suspense) => Some(suspense),
                _ => None,
            })
            .expect("SSR MIR suspense");
        assert_eq!(suspense.slots.flag, Vue3SlotFlag::Stable);
        assert_eq!(
            suspense
                .slots
                .slots
                .iter()
                .map(|slot| slot.name.as_str())
                .collect::<Vec<_>>(),
            vec!["default", "fallback"]
        );
        assert!(result
            .mir
            .nodes
            .iter()
            .all(|node| !matches!(&node.kind, Vue3SsrMirKind::RenderComponent(component) if matches!(&component.tag, MirExpr::String(tag) if tag == "Teleport" || tag == "Suspense"))));
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["target", "off"]
        );
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_groups_if_else_branch_chains() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<p v-if="ok">yes</p><p v-else-if="maybe">maybe</p><p v-else>no</p>"#.into(),
            file_id: FileId(17),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));
        let hir_if = result
            .hir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                HirNodeKind::If(hir_if) => Some(hir_if),
                _ => None,
            })
            .expect("HIR if");
        assert_eq!(hir_if.branches.len(), 3);
        assert!(hir_if.branches[0].condition.is_some());
        assert!(hir_if.branches[1].condition.is_some());
        assert!(hir_if.branches[2].condition.is_none());
        assert_eq!(
            result
                .mir
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, Vue3SsrMirKind::If { .. }))
                .count(),
            3
        );
        let root_if = result
            .mir
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    node.kind,
                    Vue3SsrMirKind::If {
                        condition: Some(_),
                        ..
                    }
                )
            })
            .expect("root if");
        let nested_if = root_if
            .children
            .iter()
            .filter(|child_id| {
                result
                    .mir
                    .node(**child_id)
                    .is_some_and(|child| matches!(child.kind, Vue3SsrMirKind::If { .. }))
            })
            .count();
        assert_eq!(nested_if, 1);
        assert_eq!(
            result
                .js
                .expressions()
                .iter()
                .map(|entry| entry.source.as_str())
                .collect::<Vec<_>>(),
            vec!["ok", "maybe"]
        );
    }
