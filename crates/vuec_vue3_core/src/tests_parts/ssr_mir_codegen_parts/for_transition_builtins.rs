    #[test]
    fn generate_vue3_ssr_mir_emits_v_for_alias_payload_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<li v-for="(item, key, index) in list">{{ key }}:{{ index }}:{{ item.name }}</li>"#.into(),
            file_id: FileId(48),
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

        assert!(generated.code.contains("ssrRenderList as _ssrRenderList"));
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.list, (item, key, index) => {"));
        assert!(generated.code.contains("_ssrInterpolate(key)"));
        assert!(generated.code.contains("_ssrInterpolate(index)"));
        assert!(generated.code.contains("_ssrInterpolate(item.name)"));
        assert!(!generated.code.contains("_ctx.key"));
        assert!(!generated.code.contains("_ctx.index"));
        assert!(!generated.code.contains("_ctx.item"));
    }

    #[test]
    fn generate_vue3_ssr_mir_wraps_v_for_fragments_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="row, i in list"><div v-for="j in row">{{ i }},{{ j }}</div></div><template v-for="item in items"><span>{{ item }}</span></template>"#.into(),
            file_id: FileId(85),
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

        assert!(generated
            .code
            .contains("_push(`<!--[-->`)\n  _ssrRenderList(_ctx.list, (row, i) => {"));
        assert!(generated
            .code
            .contains("_push(`<div><!--[-->`)\n    _ssrRenderList(row, (j) => {"));
        assert!(generated.code.contains("_push(`<!--]--></div>`)"));
        assert!(generated
            .code
            .contains("_push(`<!--[-->`)\n  _ssrRenderList(_ctx.items, (item) => {"));
        assert!(generated
            .code
            .contains("_push(`<span>${_ssrInterpolate(item)}</span>`)"));
        assert!(!generated.code.contains("<template"));
        assert!(!generated.code.contains("_ctx.row"));
        assert!(!generated.code.contains("_ctx.j"));
        assert!(!generated.code.contains("_ssrInterpolate(_ctx.item)"));
    }

    #[test]
    fn lower_vue3_ast_to_ssr_mir_flattens_transition_group_artifacts() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<TransitionGroup tag="ul"><template v-for="item in list"><li>{{ item }}</li></template><span v-if="ok">ok</span><!--x--></TransitionGroup><template v-for="entry in entries"><i>{{ entry }}</i></template>"#.into(),
            file_id: FileId(86),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let result = lower_vue3_ast_to_ssr_mir(&ast, &Vue3CompilerOptions::default());

        assert_eq!(result.hir.validate_tree(), Ok(()));
        assert_eq!(result.mir.validate_tree(), Ok(()));

        let for_nodes = result
            .mir
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3SsrMirKind::For(for_mir) => Some((
                    result.js.expressions()[for_mir.source.0 as usize]
                        .source
                        .as_str(),
                    for_mir.fragment,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(for_nodes.len(), 2);
        assert!(
            for_nodes
                .iter()
                .any(|(source, fragment)| *source == "list" && !fragment),
            "{for_nodes:?}"
        );
        assert!(for_nodes
            .iter()
            .any(|(source, fragment)| *source == "entries" && *fragment));

        let if_node = result
            .mir
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                Vue3SsrMirKind::If {
                    condition: Some(condition),
                    comment,
                } if result.js.expressions()[condition.0 as usize].source == "ok" => Some(comment),
                _ => None,
            })
            .expect("transition-group if");
        assert!(!if_node);

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
            .contains("_ssrRenderList(_ctx.list, (item) => {"));
        assert!(generated
            .code
            .contains("_push(`<li>${_ssrInterpolate(item)}</li>`)"));
        assert!(!generated.code.contains("<ul><!--[-->"));
        assert!(!generated.code.contains("<!--x-->"));
        assert!(generated
            .code
            .contains("_ssrRenderList(_ctx.entries, (entry) => {"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_transition_group_public_shapes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Transition appear><div>foo</div></Transition><TransitionGroup :tag="someTag" class="red"><span>hello</span></TransitionGroup>"#.into(),
            file_id: FileId(87),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions {
            mode: "module".into(),
            prefix_identifiers: true,
            scope_id: Some("data-v-xxxxxxx".into()),
            ..Vue3CompilerOptions::default()
        };
        let ast = Vue3Dialect::base_parse(source, &options);
        let result = lower_vue3_ast_to_ssr_mir(&ast, &options);
        let generated = generate_vue3_ssr_mir(&result.mir, &result.js, &options);

        assert!(
            generated
                .code
                .contains("<template><div data-v-xxxxxxx>foo</div></template>"),
            "{}",
            generated.code
        );
        assert!(generated.code.contains("_ctx.someTag"));
        assert!(generated
            .code
            .contains("_ssrRenderAttrs({ class: \"red\" })"));
        assert!(
            generated
                .code
                .contains("} data-v-xxxxxxx><span data-v-xxxxxxx>hello</span>"),
            "{}",
            generated.code
        );
        assert!(!generated.code.contains("data-vuec-slotted"));
        assert!(!generated.code.contains("<#expr"));
    }

    #[test]
    fn generate_vue3_ssr_mir_emits_builtin_component_payloads_from_mir() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r##"<Teleport to="#modal" disabled><div id="x"/></Teleport><Suspense><template #default><Foo/></template><template #fallback>loading</template></Suspense>"##.into(),
            file_id: FileId(49),
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

        assert!(generated
            .code
            .contains("ssrRenderTeleport as _ssrRenderTeleport"));
        assert!(generated
            .code
            .contains("ssrRenderSuspense as _ssrRenderSuspense"));
        assert!(generated
            .code
            .contains("_ssrRenderTeleport(_push, (_push) => {"));
        assert!(generated.code.contains("}, \"#modal\", true, _parent)"));
        assert!(!generated.code.contains("}, \"#modal\", true, _parent);"));
        assert!(generated.code.contains("withCtx as _withCtx"));
        assert!(generated.code.contains("_ssrRenderSuspense(_push, {"));
        assert!(generated.code.contains("default: () => {"));
        assert!(generated.code.contains("fallback: () => {"));
        assert!(generated.code.contains("_: 1 /* STABLE */"));
        assert!(generated.code.contains("_push(`loading`)"));
        assert!(!generated.code.contains("_push(\"loading\");"));
        assert!(!generated.code.contains("});"));
        assert!(!generated.code.contains("_ssrRenderComponent(\"Teleport\""));
        assert!(!generated.code.contains("_ssrRenderComponent(\"Suspense\""));
    }
