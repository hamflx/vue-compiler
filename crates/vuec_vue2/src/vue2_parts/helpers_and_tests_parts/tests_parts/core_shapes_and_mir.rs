    #[test]
    fn compile_returns_vue2_shapes() {
        let result = compile("<div>{{ msg }}</div>", options());
        assert!(result.render.contains("with(this)"));
        assert!(result.render.contains("_s(msg)"));
        assert!(result.ast.node(result.ast.root).is_some());
        assert!(result.element_ast.is_some());
    }
    #[test]
    fn compile_to_functions_wraps_render() {
        let result = compile_to_functions("<div/>", options());
        assert!(result.render.contains("with(this)"));
    }
    #[test]
    fn projects_vue2_public_ast_with_registered_js_ids() {
        let compiled = compile(
            r#"<div :id="item.id" @click.stop="save(item)">{{ item.name | upper }}</div>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<div/>", compiled.element_ast.as_ref());
        assert!(projected.ast.validate_span_consistency().is_ok());
        assert!(projected.js.expressions().len() >= 2);
        assert_eq!(projected.js.statements().len(), 1);

        let root = projected.ast.root_node().unwrap();
        let element = projected.ast.node(root.children[0]).unwrap();
        let Vue2AstKind::Element(element) = &element.kind else {
            panic!("expected Vue2 element projection");
        };
        assert!(element.attrs.iter().any(|attr| attr.name == "id"));
        assert_eq!(
            element
                .events
                .get("click")
                .and_then(|handlers| handlers.first())
                .map(|handler| handler.modifiers.contains_key("stop")),
            Some(true)
        );

        let text_id = projected.ast.node(root.children[0]).unwrap().children[0];
        let Vue2AstKind::ExpressionText(text) = &projected.ast.node(text_id).unwrap().kind else {
            panic!("expected expression text projection");
        };
        let filter = text.filter_expr.as_ref().expect("Vue2 filter payload");
        assert_eq!(filter.filters[0].name, "upper");
    }

    #[test]
    fn lowers_vue2_ast_to_hir_and_target_split_mir() {
        let compiled = compile(
            r#"<ul><li v-for="(item, i) in items" :key="item.id">{{ item.name }}</li></ul>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<ul/>", compiled.element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);

        assert!(lowered.hir.validate_span_consistency().is_ok());
        assert!(lowered.mir.validate_span_consistency().is_ok());
        assert_eq!(
            lowered
                .map
                .hir_for_ast(projected.ast.root)
                .collect::<Vec<_>>(),
            vec![lowered.hir.root]
        );
        assert!(lowered.map.hir_to_mir.iter().any(|(_, mir)| matches!(
            lowered.mir.node(*mir).map(|node| &node.kind),
            Some(Vue2MirKind::For { .. })
        )));
        assert!(lowered.hir.nodes.iter().any(|node| matches!(
            node.kind,
            HirNodeKind::For(_) | HirNodeKind::Interpolation(_)
        )));
        assert!(lowered.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue2MirKind::CreateElement(_) | Vue2MirKind::Text(_)
        )));
    }

    #[test]
    fn lowers_vue2_if_chain_and_filters_without_hir_helpers() {
        let compiled = compile(
            r#"<div><p v-if="ok">{{ msg | upper }}</p><p v-else>fallback</p></div>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<div/>", compiled.element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);

        let if_nodes = lowered
            .hir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                HirNodeKind::If(if_node) => Some(if_node),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(if_nodes.len(), 1);
        assert_eq!(if_nodes[0].branches.len(), 2);
        assert!(lowered.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue2MirKind::If { .. } | Vue2MirKind::FilterCall { .. }
        )));
        assert!(lowered.hir.nodes.iter().all(|node| {
            !matches!(
                node.kind,
                HirNodeKind::Fragment(_) if lowered.mir.node(node.id).is_some()
            )
        }));
    }

    #[test]
    fn lowers_vue2_codegen_payloads_into_mir() {
        let compiled = compile(
            r#"<section><button @click.stop="save" :id="foo">go</button><foo><template #default="slotProps">{{ slotProps.msg }}</template></foo></section>"#,
            options(),
        );
        let projected = project_vue2_public_ast("<section/>", compiled.element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);

        let button_data = lowered.mir.nodes.iter().find_map(|node| match &node.kind {
            Vue2MirKind::CreateElement(create)
                if matches!(&create.tag, MirExpr::String(tag) if tag == "button") =>
            {
                create.data.as_ref()
            }
            _ => None,
        });
        let button_data = button_data.expect("button MIR data");
        assert!(button_data.attrs.iter().any(|attr| attr.name == "id"));
        assert!(button_data
            .events
            .get("click")
            .and_then(|handlers| handlers.first())
            .is_some_and(|handler| handler.modifiers.contains_key("stop")));

        let scoped = lowered.mir.nodes.iter().find_map(|node| match &node.kind {
            Vue2MirKind::CreateElement(create)
                if matches!(&create.tag, MirExpr::String(tag) if tag == "foo") =>
            {
                create
                    .data
                    .as_ref()
                    .and_then(|data| data.scoped_slots.first())
            }
            _ => None,
        });
        let scoped = scoped.expect("scoped slot payload");
        assert!(scoped.new_syntax);
        assert!(scoped.body_is_fragment);
        assert!(!scoped.body.is_empty());

        let static_compiled = compile(r#"<div v-pre><p>{{ msg }}</p></div>"#, options());
        let static_projected =
            project_vue2_public_ast("<div/>", static_compiled.element_ast.as_ref());
        let static_lowered = lower_vue2_ast_to_mir(&static_projected.ast, static_projected.js);
        assert!(static_lowered.mir.nodes.iter().any(|node| matches!(
            node.kind,
            Vue2MirKind::RenderStatic(Vue2RenderStatic { body: Some(_), .. })
        )));
    }

    #[test]
    fn normalizes_if_template_with_nested_for_children() {
        let result = compile(
            r#"<div><template v-if="ok"><foo v-for="i in 1" :key="i"></foo></template></div>"#,
            options(),
        );

        assert_eq!(
            result.render,
            "with(this){return _c('div',[(ok)?_l((1),function(i){return _c('foo',{key:i})}):_e()],2)}"
        );
    }

    #[test]
    fn normalizes_legacy_named_slot_template_children() {
        let result = compile(
            r#"<Alert><template slot="desc">Content</template></Alert>"#,
            options(),
        );

        assert_eq!(
            result.render,
            r#"with(this){return _c('Alert',[_c('template',{slot:"desc"},[_v("Content")])],2)}"#
        );

        let conditional = compile(
            r#"<Alert v-if="show"><template slot="desc">Content</template></Alert>"#,
            options(),
        );
        assert_eq!(
            conditional.render,
            r#"with(this){return (show)?_c('Alert',[_c('template',{slot:"desc"},[_v("Content")])],2):_e()}"#
        );
    }

    #[test]
    fn hoists_vue2_once_for_list_like_official_codegen() {
        let result = compile(
            r#"<div><i :class="`${prefix}-bar`" v-once v-for="i in 8" :key="`trigger-${i}`"></i></div>"#,
            options(),
        );

        assert_eq!(result.render, r#"with(this){return _c('div',_m(0),0)}"#);
        assert_eq!(
            result.static_render_fns,
            vec![
                r#"with(this){return _l((8),function(i){return _c('i',{key:`trigger-${i}`,class:`${prefix}-bar`})})}"#
            ]
        );

        let nested_for = compile(
            r#"<div v-for="j in 2"><i v-once v-for="i in 8" :key="i">x</i></div>"#,
            options(),
        );
        assert_eq!(
            nested_for.render,
            r#"with(this){return _l((2),function(j){return _c('div',_l((8),function(i){return _c('i',{key:i},[_v("x")])}),0)})}"#
        );
        assert!(nested_for.static_render_fns.is_empty());
    }

    #[test]
    fn parses_v_for_and_generates_list_render() {
        let result = compile(
            r#"<div><li v-for="(item, i) in items" :key="item.uid">{{ item }}</li></div>"#,
            options(),
        );
        assert!(result.render.contains("_l((items),function(item,i)"));
        assert!(result.render.contains("key:item.uid"));
    }

    #[test]
    fn parses_v_if_else_chain() {
        let result = compile(
            r#"<div><p v-if="show">hello</p><p v-else>world</p></div>"#,
            options(),
        );
        assert!(result.render.contains("(show)?_c('p'"), "{}", result.render);
        assert!(result.render.contains(":_c('p'"), "{}", result.render);
    }

    #[test]
    fn lowers_vue2_v_for_on_else_branch() {
        let result = compile(
            r#"<div><p v-if="empty">empty</p><p v-for="row in rows" v-else :key="row.id">{{ row.name }}</p></div>"#,
            options(),
        );

        assert_eq!(
            result.render,
            r#"with(this){return _c('div',[(empty)?_c('p',[_v("empty")]):_l((rows),function(row){return _c('p',{key:row.id},[_v(_s(row.name))])})],2)}"#
        );
    }

    #[test]
    fn generates_filters_and_events() {
        let result = compile(
            r#"<div :id="a | b | c" @click.stop="save">{{ d | e }}</div>"#,
            options(),
        );
        assert!(result.render.contains("_f(\"c\")(_f(\"b\")(a))"));
        assert!(result.render.contains("$event.stopPropagation();"));

        let spaced_filter = compile("<div>\n  {{ d | e }}\n</div>", options());
        assert_eq!(
            spaced_filter.render,
            "with(this){return _c('div',[_v(\"\\n  \"+_s(_f(\"e\")(d))+\"\\n\")])}"
        );
    }

    #[test]
    fn preserves_multiline_bound_attribute_expressions() {
        let result = compile(
            "<Radio\n\t:options=\"[\n\t\t{ text: 'hello', value: 1 },\n\t\t{ text: 'world', value: 2 }\n\t]\"\n/>",
            options(),
        );

        assert_eq!(
            result.render,
            concat!(
                "with(this){return _c('Radio',{attrs:{\"options\":[\n",
                "\t\t{ text: 'hello', value: 1 },\n",
                "\t\t{ text: 'world', value: 2 }\n",
                "\t]}})}"
            )
        );
        assert!(!result.render.contains("\\n"));
    }
